pub mod clipboard;
use clipboard::initialize_clipboard;
pub mod common;
use common::{ask_for_a_high_resolution_timer, backend_add_log, TimeMonitor, log_lock_error_void, to_frontend_update_keyboard_devices, to_frontend_update_mouse_devices, to_frontend_update_self_app, to_frontend_update_discovered_apps, to_frontend_auto_update_discovered_apps};
pub mod device_names;
pub mod focus;
use focus::{broadcast_set_of_monitors, send_focus_with_position};
pub mod login;
use login::{auto_connect_to_app, send_disconnect_from_app, send_connect_to_app};
pub mod keyboards;
use keyboards::{discover_available_keyboards, update_active_keyboards, execute_keyboard_events, fetch_keyboard_events};
pub mod mouses;
use mouses::{discover_available_mouses, fetch_self_monitors, apply_new_borders, update_set_of_monitors, update_active_mouses, execute_mouse_events, get_all_apps_monitors, fetch_mouse_events};
pub mod networking;
use networking::{networking_loop, submit_network_request_locking, submit_broadcast_network_request};
pub mod states;
use states::{BackendGlobalState, HardDiskStorageResponse, LogLevel, AppResponse, ActiveKeyboardBackendResponse, ActiveMouseBackendResponse,
    DragBackendResponse, SubmitAppNetworkConfigBackendResponse, SetSelfOnlineBackendResponse, SetFocusedIdBackendResponse,
    ConnectToAppBackendResponse, DisconnectToAppBackendResponse, SetOfMonitors, Monitor, EditMonitorsBackendResponse, StoredOtherApp,
    NetworkAction, ConnectRequestContent, DisconnectRequestContent, BroadcastRequestContent, NetworkApplicationRequest, NetworkApplicationBroadcastRequest};
pub mod storage;
use storage::{CONFIG_FILENAME, get_config_directory, load_config_or_set_default, save_config, update_application_config};

use std::sync::{Arc, Mutex, OnceLock};

use tauri::{AppHandle, Manager};
use tauri_plugin_log::{Target, TargetKind};


#[tauri::command(rename_all = "snake_case")]
fn get_config_path() -> String {
    match get_config_directory() {
        Ok(config_directory) => {
            let config_path = config_directory.join(CONFIG_FILENAME);
            config_path.to_string_lossy().to_string()
        },
        Err(err) => {
            err
        }
    }
}

#[tauri::command(rename_all = "snake_case")]
fn frontend_ready() -> Result<(), ()> {
    let app_handle = get_handle();
    {
        let state = app_handle.state::<Arc<Mutex<BackendGlobalState>>>();
        let mut state = match state.lock() {
            Ok(state) => state,
            Err(err) => { log_lock_error_void(format!("Lock error when frontend is ready: {err}"), &app_handle); return Err(()); }, // Early return
        };
        state.frontend_ready = true;
    }
    return_back_handle(app_handle);
    Ok(())
}

#[tauri::command(rename_all = "snake_case")]
async fn refresh_self_app() -> Result<(), ()> {
    log::debug!("backend: refresh_self_app");
    let app_handle = get_handle();

    let mut content = BroadcastRequestContent {
        name: "".to_string(),
        online: false,
        connected_ids: vec!(),
        set_of_monitors: SetOfMonitors::default(),
    };

    // Ensures there is no deadlock
    {
        let state = app_handle.state::<Arc<Mutex<BackendGlobalState>>>();
        let state = match state.lock() {
            Ok(state) => state,
            Err(err) => { log_lock_error_void(format!("Lock error when attempting to refresh_self_app: {err}"), &app_handle); return Err(()); }, // Early return
        };
        let network_info = &state.network_info;

        content.name = network_info.self_info.name.clone();
        content.online = network_info.self_info.online;
        content.connected_ids = network_info.self_info.connected_ids.clone();
        content.set_of_monitors = network_info.self_info.set_of_monitors.clone();
        let response: AppResponse = AppResponse::from(&network_info.self_info);
        drop(state); // For optimisation
        to_frontend_update_self_app(response, &app_handle);
    }

    let request_content = match postcard::to_allocvec(&content) {
        Ok(request_content) => request_content,
        Err(_) => { // Should never happen
            return Err(()); // Early return
        },
    };
    submit_broadcast_network_request(NetworkApplicationBroadcastRequest {
        action: NetworkAction::Broadcast,
        content: request_content,
    }).await;
    return_back_handle(app_handle);
    Ok(())
}

#[tauri::command(rename_all = "snake_case")]
async fn refresh_discovered_apps() -> Result<(), ()> {
    log::debug!("backend: refresh_discovered_apps");
    let app_handle = get_handle();
    to_frontend_auto_update_discovered_apps(&app_handle).await;
    return_back_handle(app_handle);
    Ok(())
}

#[tauri::command(rename_all = "snake_case")]
async fn submit_app_network_config(config: &str) -> Result<(), ()> {
    log::debug!("backend: submit_app_network_config");
    let app_handle = get_handle();

    let config: SubmitAppNetworkConfigBackendResponse = match serde_json::from_str(config) {
        Ok(config) => config,
        Err(err) => {
            let error = format!("Parsing bug when trying to submit_app_network_config: {}", err);
            log::error!("{}", error.to_string());
            backend_add_log(error.to_string(), LogLevel::Error, &app_handle);
            return Err(()); // Early return
        },
    };

    let state = app_handle.state::<Arc<Mutex<BackendGlobalState>>>();
    let mut state = match state.lock() {
        Ok(state) => state,
        Err(err) => { log_lock_error_void(format!("Lock error when attempting to submit_app_network_config: {err}"), &app_handle); return Err(()); }, // Early return
    };
    let mut app_name = "Other app".to_string();
    if let Some(app) = state.network_info.discovered_apps.get_mut(&config.id) {
        app.info.password = config.password.clone();
        app_name = app.info.name.clone();
    }

    // App password is memorized
    if let Some(app) = state.hard_disk_storage.other_apps.iter_mut().find(|app| app.id == *config.id) {
        app.password = config.password;
    } else {
        state.hard_disk_storage.other_apps.push(StoredOtherApp {
            id: config.id,
            app_name,
            auto_connect: false,
            password: config.password,
        });
    }

    let response: Vec<AppResponse> = state.network_info.discovered_apps.values().map(|app|
        AppResponse::from(&app.info)).collect();
    let config = state.hard_disk_storage.clone();
    let _ = save_config(&app_handle, config);
    drop(state); // For optimisation

    to_frontend_update_discovered_apps(response, &app_handle);
    return_back_handle(app_handle);
    Ok(())
}

#[tauri::command(rename_all = "snake_case")]
async fn submit_edit_monitors(edit: &str) -> Result<(), ()> {
    log::debug!("backend: submit_edit_monitors");
    let app_handle = get_handle();

    let mut edit: EditMonitorsBackendResponse = match serde_json::from_str(edit) {
        Ok(config) => config,
        Err(err) => {
            let error = format!("Parsing bug when trying to submit_edit_monitors: {}", err);
            log::error!("{}", error.to_string());
            backend_add_log(error.to_string(), LogLevel::Error, &app_handle);
            return Err(()); // Early return
        },
    };

    let state = app_handle.state::<Arc<Mutex<BackendGlobalState>>>();
    let mut state = match state.lock() {
        Ok(state) => state,
        Err(err) => { log_lock_error_void(format!("Lock error when attempting to submit_edit_monitors: {err}"), &app_handle); return Err(()); }, // Early return
    };

    // Set monitors_id for borders
    let apps_monitors = get_all_apps_monitors(&state.network_info);
    for border in edit.borders.iter_mut() {
        if let Some(app_monitors) = apps_monitors.iter().find(|app_monitors| app_monitors.id == border.pair[0].app_id) {
            border.pair[0].monitors_id = Monitor::get_monitors_id(&app_monitors.set_of_monitors.monitors);
        }
        if let Some(app_monitors) = apps_monitors.iter().find(|app_monitors| app_monitors.id == border.pair[1].app_id) {
            border.pair[1].monitors_id = Monitor::get_monitors_id(&app_monitors.set_of_monitors.monitors);
        }
    }

    apply_new_borders(&mut state, edit.borders, edit.apps, &app_handle);

    broadcast_set_of_monitors(&mut state.network_info);

    let self_response: AppResponse = AppResponse::from(&state.network_info.self_info);
    let response: Vec<AppResponse> = state.network_info.discovered_apps.values().map(|app|
        AppResponse::from(&app.info)).collect();
    drop(state); // For optimisation
    to_frontend_update_self_app(self_response, &app_handle);
    to_frontend_update_discovered_apps(response, &app_handle);

    return_back_handle(app_handle);
    Ok(())
}

#[tauri::command(rename_all = "snake_case")]
async fn request_clipboard(peer_id: &str) -> Result<(), ()> {
    log::debug!("backend: request_clipboard");
    let peer_id = peer_id.to_string();
    let handles = submit_network_request_locking(NetworkApplicationRequest {
        to_id: peer_id,
        action: NetworkAction::FetchClipboardEvent,
        content: vec!(),
    });
    for handle in handles {
        let _ = handle.await;
    }
    Ok(())
}

#[tauri::command(rename_all = "snake_case")]
async fn set_self_online(online: &str) -> Result<(), ()> {
    log::debug!("backend: set_self_online");
    let app_handle = get_handle();

    let online: SetSelfOnlineBackendResponse = match serde_json::from_str(online) {
        Ok(online) => online,
        Err(err) => {
            let error = format!("Parsing bug when trying to set_self_online: {}", err);
            log::error!("{}", error.to_string());
            backend_add_log(error.to_string(), LogLevel::Error, &app_handle);
            return Err(()); // Early return
        },
    };

    let name;
    let connected_ids;
    let set_of_monitors;
    // Ensures that there is no deadlock
    {
        let state = app_handle.state::<Arc<Mutex<BackendGlobalState>>>();
        let mut state = match state.lock() {
            Ok(state) => state,
            Err(err) => { log_lock_error_void(format!("Lock error when attempting to set_self_online: {err}"), &app_handle); return Err(()); }, // Early return
        };
        state.network_info.self_info.online = online.online;
        name = state.network_info.self_info.name.clone();
        connected_ids = state.network_info.self_info.connected_ids.clone();
        set_of_monitors = state.network_info.self_info.set_of_monitors.clone();

        // Online configuration is memorized
        state.hard_disk_storage.online = online.online;

        let response: AppResponse = AppResponse::from(&state.network_info.self_info);
        let config = state.hard_disk_storage.clone();
        let _ = save_config(&app_handle, config);
        drop(state); // For optimisation

        to_frontend_update_self_app(response, &app_handle);
    }

    let content = BroadcastRequestContent {
        name,
        online: online.online,
        connected_ids,
        set_of_monitors,
    };
    let request_content = match postcard::to_allocvec(&content) {
        Ok(request_content) => request_content,
        Err(_) => { // Should never happen
            return Err(()); // Early return
        },
    };
    submit_broadcast_network_request(NetworkApplicationBroadcastRequest {
        action: NetworkAction::Broadcast,
        content: request_content,
    }).await;
    return_back_handle(app_handle);
    Ok(())
}

#[tauri::command(rename_all = "snake_case")]
async fn set_focused_id(focus: &str) -> Result<(), ()> {
    log::debug!("backend: set_focused_id");
    let app_handle = get_handle();

    let focus: SetFocusedIdBackendResponse = match serde_json::from_str(focus) {
        Ok(focus) => focus,
        Err(err) => {
            let error = format!("Parsing bug when trying to set_focused_id: {}", err);
            log::error!("{}", error.to_string());
            backend_add_log(error.to_string(), LogLevel::Error, &app_handle);
            return Err(()); // Early return
        },
    };

    let state = app_handle.state::<Arc<Mutex<BackendGlobalState>>>();
    let mut state = match state.lock() {
        Ok(state) => state,
        Err(err) => { log_lock_error_void(format!("Lock error when attempting to set_focused_id: {err}"), &app_handle); return Err(()); }, // Early return
    };
    let focused_id = focus.focused_id;
    let network_info = &mut state.network_info;

    send_focus_with_position(focused_id, xavkeyboardandmousegrabber::MouseMovement { x: focus.x, y: focus.y }, network_info, &app_handle);

    drop(state);
    return_back_handle(app_handle);
    Ok(())
}

#[tauri::command(rename_all = "snake_case")]
async fn connect_to_app(app: &str) -> Result<(), ()> {
    log::debug!("backend: connect_to_app");
    let app_handle = get_handle();

    let app_to_connect: ConnectToAppBackendResponse = match serde_json::from_str(app) {
        Ok(app_to_connect) => app_to_connect,
        Err(err) => {
            let error = format!("Parsing bug when trying to connect to app: {}", err);
            log::error!("{}", error.to_string());
            backend_add_log(error.to_string(), LogLevel::Error, &app_handle);
            return Err(()); // Early return
        },
    };

    let content = ConnectRequestContent {
        password: app_to_connect.password,
    };
    send_connect_to_app(&app_to_connect.id, content, &app_handle);

    let message = format!("Attempting to connect directly to {}", app_to_connect.id);
    log::info!("{}", message);
    backend_add_log(message, LogLevel::Info, &app_handle);

    return_back_handle(app_handle);
    Ok(())
}

#[tauri::command(rename_all = "snake_case")]
async fn submit_config(partial_config: &str) -> Result<(), ()> {
    log::debug!("backend: submit_config");
    let app_handle = get_handle();

    let config: HardDiskStorageResponse = match serde_json::from_str(partial_config) {
        Ok(app_to_connect) => app_to_connect,
        Err(err) => {
            let error = format!("Parsing bug when trying to submit config: {}", err);
            log::error!("{}", error.to_string());
            backend_add_log(error.to_string(), LogLevel::Error, &app_handle);
            return Err(()); // Early return
        },
    };

    let state = app_handle.state::<Arc<Mutex<BackendGlobalState>>>();
    let mut state = match state.lock() {
        Ok(state) => state,
        Err(err) => { log_lock_error_void(format!("Lock error when attempting to submit config: {err}"), &app_handle); return Err(()); }, // Early return
    };
    // Only apply defined values
    if let Some(app_name) = &config.app_name {
        state.hard_disk_storage.app_name = app_name.clone();
    }
    if let Some(password) = &config.password {
        state.hard_disk_storage.password = password.clone();
    }
    if let Some(theme) = &config.theme {
        state.hard_disk_storage.theme = theme.clone();
    }
    if let Some(default_width) = config.default_width {
        state.hard_disk_storage.default_width = default_width;
    }
    if let Some(default_height) = config.default_height {
        state.hard_disk_storage.default_height = default_height;
    }
    if let Some(zoom) = config.zoom {
        state.hard_disk_storage.zoom = zoom;
    }
    if let Some(enable_clipboard) = config.enable_clipboard {
        state.hard_disk_storage.enable_clipboard = enable_clipboard;
    }
    if let Some(maximum_logs) = config.maximum_logs {
        state.hard_disk_storage.maximum_logs = maximum_logs;
    }
    if let Some(auto_connect) = config.auto_connect {
        state.hard_disk_storage.auto_connect = auto_connect;
    }
    if let Some(download_path) = &config.download_path {
        state.hard_disk_storage.download_path = download_path.clone();
    }
    // keypair, online, remembered_keyboards and remembered_mouses are updated elsewhere
    drop(state);
    update_application_config(Some(config), &app_handle);

    let state = app_handle.state::<Arc<Mutex<BackendGlobalState>>>();
    let state = match state.lock() {
        Ok(state) => state,
        Err(err) => { log_lock_error_void(format!("Lock error when attempting to save config: {err}"), &app_handle); return Err(()); }, // Early return
    };
    let config = state.hard_disk_storage.clone();
    let _ = save_config(&app_handle, config);

    drop(state);
    return_back_handle(app_handle);
    Ok(())
}

#[tauri::command(rename_all = "snake_case")]
async fn disconnect_from_app(app: &str) -> Result<(), ()> {
    log::debug!("backend: disconnect_from_app");
    let app_handle = get_handle();

    let app_to_disconnect: DisconnectToAppBackendResponse = match serde_json::from_str(app) {
        Ok(app_to_disconnect) => app_to_disconnect,
        Err(err) => {
            let error = format!("Parsing bug when trying to disconnect from app: {}", err);
            log::error!("{}", error.to_string());
            backend_add_log(error.to_string(), LogLevel::Error, &app_handle);
            return Err(()); // Early return
        },
    };

    let state = app_handle.state::<Arc<Mutex<BackendGlobalState>>>();
    let mut state = match state.lock() {
        Ok(state) => state,
        Err(err) => { log_lock_error_void(format!("Lock error when attempting to send_connect_to_app: {err}"), &app_handle); return Err(()); }, // Early return
    };
    let network_info = &mut state.network_info;
    send_disconnect_from_app(
        &app_to_disconnect.id,
        Some(DisconnectRequestContent {
            is_manual_disconnect: true,
            is_refused: false,
        }),
        network_info,
        &app_handle
    );

    // For a manual disconnection, forget an app for auto connection
    if let Some(app) = state.hard_disk_storage.other_apps.iter_mut().find(|app| app.id == *app_to_disconnect.id) {
        app.auto_connect = false;
    }
    let config = state.hard_disk_storage.clone();
    let _ = save_config(&app_handle, config);
    drop(state);

    let message = format!("Disconnected directly from {}", app_to_disconnect.id);
    log::info!("{}", message);
    backend_add_log(message, LogLevel::Info, &app_handle);

    return_back_handle(app_handle);
    Ok(())
}

#[tauri::command(rename_all = "snake_case")]
async fn update_keyboards(updated_keyboards: &str) -> Result<(), ()> {
    log::debug!("backend: update_keyboards");
    let app_handle = get_handle();

    let updated_keyboards: Vec<ActiveKeyboardBackendResponse> = match serde_json::from_str(updated_keyboards) {
        Ok(updated_keyboards) => updated_keyboards,
        Err(err) => {
            let error = format!("Parsing bug when trying to update active keyboards: {}", err);
            log::error!("{}", error.to_string());
            backend_add_log(error.to_string(), LogLevel::Error, &app_handle);
            return Err(()); // Early return
        },
    };

    let _ = update_active_keyboards(updated_keyboards, &app_handle);
    return_back_handle(app_handle);
    Ok(())
}

#[tauri::command(rename_all = "snake_case")]
async fn update_mouses(updated_mouses: &str) -> Result<(), ()> {
    log::debug!("backend: update_mouses");
    let app_handle = get_handle();

    let updated_mouses: Vec<ActiveMouseBackendResponse> = match serde_json::from_str(updated_mouses) {
        Ok(updated_mouses) => updated_mouses,
        Err(err) => {
            let error = format!("Parsing bug when trying to update active mouses: {}", err);
            log::error!("{}", error.to_string());
            backend_add_log(error.to_string(), LogLevel::Error, &app_handle);
            return Err(()); // Early return
        },
    };

    let _ = update_active_mouses(updated_mouses, &app_handle);
    return_back_handle(app_handle);
    Ok(())
}

#[tauri::command(rename_all = "snake_case")]
async fn refresh_keyboards() -> Result<(), ()> {
    log::debug!("backend: refresh_keyboard");
    let app_handle = get_handle();

    let state = app_handle.state::<Arc<Mutex<BackendGlobalState>>>();
    let mut state = match state.lock() {
        Ok(state) => state,
        Err(err) => { log_lock_error_void(format!("Lock error when attempting to refresh frontend keyboards: {err}"), &app_handle); return Err(()); }, // Early return
    };
    let keyboards = &mut state.keyboards_info_map;

    let response: Vec<ActiveKeyboardBackendResponse> = keyboards.values().map(|keyboard_info| ActiveKeyboardBackendResponse {
        name: keyboard_info.keyboard.device_name.to_string(),
        id: keyboard_info.keyboard.device_path.to_string(),
        active: keyboard_info.active,
    }).collect();
    drop(state); // For optimisation
    to_frontend_update_keyboard_devices(response, &app_handle);
    return_back_handle(app_handle);
    Ok(())
}

#[tauri::command(rename_all = "snake_case")]
async fn refresh_mouses() -> Result<(), ()> {
    log::debug!("backend: refresh_mouse");
    let app_handle = get_handle();

    let state = app_handle.state::<Arc<Mutex<BackendGlobalState>>>();
    let mut state = match state.lock() {
        Ok(state) => state,
        Err(err) => { log_lock_error_void(format!("Lock error when attempting to refresh frontend mouses: {err}"), &app_handle); return Err(()); }, // Early return
    };
    let mouses = &mut state.mouses_info_map;

    let response: Vec<ActiveMouseBackendResponse> = mouses.values().map(|mouse_info| ActiveMouseBackendResponse {
        name: mouse_info.mouse.device_name.to_string(),
        id: mouse_info.mouse.device_path.to_string(),
        active: mouse_info.active,
    }).collect();
    drop(state); // For optimisation
    to_frontend_update_mouse_devices(response, &app_handle);
    return_back_handle(app_handle);
    Ok(())
}

#[tauri::command(rename_all = "snake_case")]
async fn transfer_files(drag: &str) -> Result<(), ()> {
    log::debug!("backend: transfer_files");
    let app_handle = get_handle();

    let drag: DragBackendResponse = match serde_json::from_str(drag) {
        Ok(drag) => drag,
        Err(err) => {
            let error = format!("Parsing bug when trying to transfer files: {}", err);
            log::error!("{}", error.to_string());
            backend_add_log(error.to_string(), LogLevel::Error, &app_handle);
            return Err(()); // Early return
        },
    };

    storage::transfer_files(drag).await;

    return_back_handle(app_handle);
    Ok(())
}

/// Prevents some Tauri bugs from crashing the program. Some functions from app.handle are not thread safe.
///
/// For more details, check: https://github.com/tauri-apps/tauri/issues/15170
pub static PREVENT_TAURI_CRASH: OnceLock::<Mutex<()>> = OnceLock::new();

/// Because tauri app_handle clone can crash when it is dropped, using a pool as a workaround.
pub static APP_HANDLES_POOL: OnceLock::<Arc<Mutex<Vec<AppHandle>>>> = OnceLock::new();
pub const APP_HANDLES_POOL_SIZE: usize = 100;

/// Workaround to get a Tauri app handle, without crash. Before the program starts, APP_HANDLES_POOL must be initialized with valid app handles.
pub fn get_handle() -> AppHandle {
    let app_handles_pool = APP_HANDLES_POOL.get_or_init(|| Arc::new(Mutex::new(Vec::with_capacity(APP_HANDLES_POOL_SIZE))));
    match app_handles_pool.lock() {
        Ok(mut app_handles_pool) => {
            if app_handles_pool.len() == 1 {
                let app_handle = app_handles_pool.pop().unwrap();
                let new_app_handle = app_handle.clone();
                app_handles_pool.push(new_app_handle);
                return app_handle;
            }

            app_handles_pool.pop().expect("APP_HANDLES_POOL should never be empty!")
        },
        Err(err) => { panic!("Fatal error: cannot get handle: {err}"); },
    }
}

/// Workaround to prevent Tauri handle from dropping. Should be called before an AppHandle is dropped.
pub fn return_back_handle(app_handle: AppHandle) {
    let app_handles_pool = APP_HANDLES_POOL.get_or_init(|| Arc::new(Mutex::new(Vec::with_capacity(APP_HANDLES_POOL_SIZE))));
    match app_handles_pool.lock() {
        Ok(mut app_handles_pool) => {
            app_handles_pool.push(app_handle);
        },
        Err(err) => { log_lock_error_void(format!("Lock error when returning handle: {err}"), &app_handle); },
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Before anything schedules a timer, see ask_for_a_high_resolution_timer
    ask_for_a_high_resolution_timer();

    tauri::Builder::default()
        .plugin(tauri_plugin_log::Builder::new()
            .level(log::LevelFilter::Info) // Debug has a large performance penalty
            .targets([
            Target::new(TargetKind::Stdout),
            Target::new(TargetKind::Webview),
        ]).build())
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_os::init())
        .invoke_handler(tauri::generate_handler![
            get_config_path, frontend_ready,
            refresh_self_app, refresh_discovered_apps, submit_app_network_config, submit_edit_monitors,
            set_self_online, set_focused_id, request_clipboard,
            connect_to_app, disconnect_from_app, submit_config,
            update_keyboards, update_mouses, refresh_keyboards, refresh_mouses, transfer_files
        ])
        .setup(|app| {
            log::debug!("backend setup");

            let global_state= BackendGlobalState::default();
            app.manage(Arc::new(Mutex::new(global_state)));
            
            let app_handle = app.handle().clone();

            // Workaround for the possibility of Tauri's handles crashing when dropped.
            let app_handles_pool = APP_HANDLES_POOL.get_or_init(|| Arc::new(Mutex::new(Vec::with_capacity(APP_HANDLES_POOL_SIZE))));
            match app_handles_pool.lock() {
                Ok(mut app_handles_pool) => {
                    for _ in 0..APP_HANDLES_POOL_SIZE {
                        app_handles_pool.push(app_handle.clone());
                    }
                },
                Err(err) => { panic!("Fatal error: could not initialize handles: {err}"); },
            }

            // Load config
            let _ = load_config_or_set_default(&app_handle);
            let load_config_app_handle = app.handle().clone();

            tauri::async_runtime::spawn(async move {

                // Wait for frontend listeners
                // Finite loop of a maximum of ~15 seconds, to prevent infinite loop in case frontend_ready is not received.
                let frontend_ready_app_handle = app_handle.clone();
                for i in 0..150 {
                    {
                        let state = frontend_ready_app_handle.state::<Arc<Mutex<BackendGlobalState>>>();
                        let state = match state.lock() {
                            Ok(state) => state,
                            Err(err) => { log_lock_error_void(format!("Lock error before frontend is ready: {err}"), &frontend_ready_app_handle); return; }, // Early return
                        };
                        if state.frontend_ready {
                            let message = format!("Received ready signal after ~{:.1} seconds.", i as f32 / 10.0);
                            log::info!("{}", message);
                            backend_add_log(message.to_string(), LogLevel::Info, &frontend_ready_app_handle);
                            break;
                        }
                    }
                    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

                    if i == 149 {
                        let error = "Failed to receive ready signal after ~15 seconds.";
                        log::error!("{}", error);
                        backend_add_log(error.to_string(), LogLevel::Error, &frontend_ready_app_handle);
                    }
                }
                // Using a duplicated config update because tauri's frontend listeners are ready after backend.
                tokio::time::sleep(std::time::Duration::from_millis(100)).await; // Wait some more time to ensure all frontend listeners are ready.
                let _ = load_config_or_set_default(&load_config_app_handle);

                let network_app_handle = app_handle.clone();
                let _networking_thread_handle = tokio::task::spawn(async move {
                    // Using a sleep because tauri's frontend listeners are ready after backend.
                    tokio::time::sleep(std::time::Duration::from_millis(1000)).await;
                    let result = networking_loop(&network_app_handle).await;
                    if let Err(err) = result {
                        let error = format!("Critical failure, network loop failed: {}", err);
                        log::error!("{}", error);
                        backend_add_log(error.to_string(), LogLevel::Error, &network_app_handle);
                    }
                });

                initialize_clipboard(&app_handle);

                // This loop if for code that is not much impacted by blocking code.
                let blocking_loop_app_handle = app_handle.clone();
                let _blocking_loop_handle = tokio::task::spawn(async move {
                    let mut i: u64 = 0;
                    loop {
                        // Check every 1 second
                        if (i).is_multiple_of(10) {
                            let _discover_keyboards_monitor = TimeMonitor::build(1000, "Discovery of keyboards", &blocking_loop_app_handle);
                            let discover_keyboards_app_handle = get_handle();
                            let discover_keyboards_handle = tokio::task::spawn_blocking(move || {
                                match discover_available_keyboards(&discover_keyboards_app_handle) {
                                    Ok(_) => (),
                                    Err(err) => {
                                        let error = format!("Failed to discover keyboards: {}", err);
                                        log::error!("{}", error.to_string());
                                        backend_add_log(error.to_string(), LogLevel::Error, &discover_keyboards_app_handle);
                                    },
                                };
                                return_back_handle(discover_keyboards_app_handle);
                            });
                            let _ = discover_keyboards_handle.await;
                        }
                        // Check every 1 second, with offset of 500 ms
                        if (i + 5).is_multiple_of(10) {
                            let _discover_mouses_monitor = TimeMonitor::build(1000, "Discovery of mouses", &blocking_loop_app_handle);
                            let discover_mouses_app_handle = get_handle();
                            let discover_mouses_handle = tokio::task::spawn_blocking(move || {
                                match discover_available_mouses(&discover_mouses_app_handle) {
                                    Ok(_) => (),
                                    Err(err) => {
                                        let error = format!("Failed to discover mouses: {}", err);
                                        log::error!("{}", error.to_string());
                                        backend_add_log(error.to_string(), LogLevel::Error, &discover_mouses_app_handle);
                                    },
                                };
                                return_back_handle(discover_mouses_app_handle);
                            });
                            let _ = discover_mouses_handle.await;
                        }
                        // Check every 1 second, with offset of 700 ms
                        if (i + 7).is_multiple_of(10) {
                            let _auto_login_monitor = TimeMonitor::new("Discovery of mouses", &blocking_loop_app_handle);
                            let auto_login_app_handle = get_handle();
                            let auto_login_handle = tokio::task::spawn_blocking(move || {
                                {
                                    let state = auto_login_app_handle.state::<Arc<Mutex<BackendGlobalState>>>();
                                    let state = match state.lock() {
                                        Ok(state) => state,
                                        Err(err) => { log_lock_error_void(format!("Lock error before auto login: {err}"), &auto_login_app_handle); return; }, // Early return
                                    };

                                    if state.hard_disk_storage.auto_connect {
                                        let apps_to_auto_connect_with: Vec<String> = state.hard_disk_storage.other_apps.iter()
                                            .filter_map(|app| app.auto_connect.then_some(app.id.clone())).collect();
                                        drop(state);

                                        for app_to_auto_connect_with in apps_to_auto_connect_with {
                                            auto_connect_to_app(&app_to_auto_connect_with, &auto_login_app_handle);
                                        }
                                    }
                                }
                                return_back_handle(auto_login_app_handle);
                            });
                            let _ = auto_login_handle.await;
                        }
                        // Check every 2 seconds, with offset of 400 ms
                        if (i + 4).is_multiple_of(2 * 10) {
                            let _update_monitor = TimeMonitor::new("Update list of monitors", &blocking_loop_app_handle);
                            let monitors = fetch_self_monitors(&blocking_loop_app_handle);
                            let state = blocking_loop_app_handle.state::<Arc<Mutex<BackendGlobalState>>>();
                            let mut new_self_content = None;
                            match state.lock() {
                                Ok(mut state) => {
                                    let network_info = &mut (state.network_info);
                                    let has_update = update_set_of_monitors(&mut network_info.self_info.set_of_monitors, monitors);
                                    if has_update {
                                        new_self_content = Some(AppResponse::from(&network_info.self_info));
                                    }
                                },
                                Err(err) => { log_lock_error_void(format!("Lock error when attempting to update list of monitors: {err}"), &blocking_loop_app_handle); },
                            };
                            if let Some(self_content) = new_self_content {
                                to_frontend_update_self_app(self_content, &blocking_loop_app_handle);
                            }
                        }
                        // Check every 3 seconds, with offset of 900 ms
                        if (i + 9).is_multiple_of(3 * 10) {
                            let _update_monitor = TimeMonitor::new("Update discovered apps", &blocking_loop_app_handle);
                            to_frontend_auto_update_discovered_apps(&blocking_loop_app_handle).await;
                        }

                        // Check every 5 seconds, with offset of 100 ms
                        if (i + 1).is_multiple_of(5 * 10) {
                            let state = blocking_loop_app_handle.state::<Arc<Mutex<BackendGlobalState>>>();
                            let mut state = match state.lock() {
                                Ok(state) => state,
                                Err(err) => { log_lock_error_void(format!("Lock error before trying to shrink memory footprint: {err}"), &blocking_loop_app_handle); return; }, // Early return
                            };

                            // Optimize RAM memory usage without thrashing active queues
                            for app in state.network_info.discovered_apps.values_mut() {
                                if app.info.file_transfers.capacity() > 64 {
                                    app.info.file_transfers.shrink_to_fit();
                                }
                                if app.confirmed_file_chunks.capacity() > 64 {
                                    app.confirmed_file_chunks.shrink_to_fit();
                                }
                                if app.received_file_chunks.capacity() > 64 {
                                    app.received_file_chunks.shrink_to_fit();
                                }
                                if app.received_requests_queue.capacity() > 256 {
                                    app.received_requests_queue.shrink_to(64);
                                }
                                if app.requests_queue.capacity() > 256 {
                                    app.requests_queue.shrink_to(64);
                                }
                                if app.responses_queue.capacity() > 64 {
                                    app.responses_queue.shrink_to_fit();
                                }
                            }

                            // Optimize memory less for better speed
                            if state.received_keyboards_events_queue.capacity() >= 256 {
                                state.received_keyboards_events_queue.shrink_to(64);
                            }
                            if state.received_mouses_events_queue.capacity() >= 256 {
                                state.received_mouses_events_queue.shrink_to(64);
                            }
                        }

                        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                        i += 1;
                    }
                });

                let execute_received_events_app_handle = app_handle.clone();
                std::thread::spawn(move || {
                    loop {
                        // Check if there are already events to execute before blocking
                        let has_events = {
                            let state = execute_received_events_app_handle.state::<Arc<Mutex<BackendGlobalState>>>();
                            match state.lock() {
                                Ok(state) => !state.received_keyboards_events_queue.is_empty() || !state.received_mouses_events_queue.is_empty(),
                                Err(_) => false,
                            }
                        };

                        if !has_events {
                            // Wait after a request has been executed, then check if there are keyboard or mouse events to execute
                            let signal_executed_requests;
                            {
                                let state = execute_received_events_app_handle.state::<Arc<Mutex<BackendGlobalState>>>();
                                match state.lock() {
                                    Ok(state) => {
                                        signal_executed_requests = state.network_info.signal_executed_requests_queue.clone();
                                    },
                                    Err(err) => { log_lock_error_void(format!("Lock error when blocking for received events: {err}"), &execute_received_events_app_handle); return; },
                                };
                            }
                            match signal_executed_requests.0.lock() {
                                Ok(lock) => {
                                    // Use 5ms wait_timeout to prevent lost wakeups from stalling the input execution
                                    let _ = signal_executed_requests.1.wait_timeout(lock, std::time::Duration::from_millis(5));
                                },
                                Err(err) => {
                                    log_lock_error_void(format!("Lock error when blocking to wait for received events: {err}"), &execute_received_events_app_handle);
                                    return;
                                },
                            };
                        }

                        let execute_keyboard_events_monitor = TimeMonitor::new("Executing keyboard events", &execute_received_events_app_handle);
                        execute_keyboard_events(&execute_received_events_app_handle);
                        drop(execute_keyboard_events_monitor);

                        let execute_mouse_events_monitor = TimeMonitor::new("Executing mouse events", &execute_received_events_app_handle);
                        execute_mouse_events(&execute_received_events_app_handle);
                        drop(execute_mouse_events_monitor);
                    }
                });

                loop {
                    let fetch_keyboard_events_monitor = TimeMonitor::new("Fetching keyboard events", &app_handle);
                    fetch_keyboard_events(&app_handle);
                    drop(fetch_keyboard_events_monitor);

                    let fetch_mouse_events_monitor = TimeMonitor::new("Fetching mouse events", &app_handle);
                    fetch_mouse_events(&app_handle);
                    drop(fetch_mouse_events_monitor);

                    tokio::time::sleep(std::time::Duration::from_millis(1)).await;
                }
            });

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
