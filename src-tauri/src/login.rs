use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

use tauri::{AppHandle, Manager};

use crate::common::{backend_add_log, log_lock_error_void, to_frontend_update_discovered_apps, to_frontend_update_self_app};
use crate::networking::{submit_network_request};
use crate::states::{AppInfo, AppResponse, BackendGlobalState, ConnectRequestContent, DisconnectRequestContent, HardDiskStorage, LogLevel, NetworkAction, NetworkApplicationRequest, NetworkInfo, OtherAppInfo, SetOfMonitors, StoredOtherApp};
use crate::storage::save_config;


pub fn send_connect_to_app(connect_to_id: &String, content: ConnectRequestContent, app_handle: &AppHandle) {
    let request_content = match postcard::to_allocvec(&content) {
        Ok(request_content) => request_content,
        Err(_) => { // Should never happen
            return; // Early return
        },
    };

    let state = app_handle.state::<Arc<Mutex<BackendGlobalState>>>();
    let mut state = match state.lock() {
        Ok(state) => state,
        Err(err) => { log_lock_error_void(format!("Lock error when attempting to send_connect_to_app: {err}"), app_handle); return; }, // Early return
    };

    let _ = submit_network_request(NetworkApplicationRequest {
        to_id: connect_to_id.clone(),
        action: NetworkAction::Connect,
        content: request_content,
    }, &mut state.network_info);
    let mut app_name = "Other app".to_string();
    let mut password = "".to_string();
    if let Some(app) = state.network_info.discovered_apps.get_mut(connect_to_id) {
        app.info.authorized_by_self = true;
        app_name = app.info.name.clone();
        password = app.info.password.clone();
    }

    let response: Vec<AppResponse> = state.network_info.discovered_apps.values().map(|app|
        AppResponse::from(&app.info)).collect();
    // Whenever a connection attempt is made, remember it for auto connection
    if state.hard_disk_storage.auto_connect {
        if !state.hard_disk_storage.other_apps.iter().any(|app| app.id == *connect_to_id) {
            state.hard_disk_storage.other_apps.push(StoredOtherApp {
                id: connect_to_id.clone(),
                app_name,
                auto_connect: true,
                password,
            });
        } else if let Some(app) = state.hard_disk_storage.other_apps.iter_mut().find(|app| app.id == *connect_to_id) {
            app.auto_connect = true;
        }
        let config = state.hard_disk_storage.clone();
        let _ = save_config(app_handle, config);
        drop(state);
    }
    to_frontend_update_discovered_apps(response, app_handle);
}

/// If the peer to connect with is offline, nothing happens
pub fn auto_connect_to_app(connect_to_id: &String, app_handle: &AppHandle) {
    let state = app_handle.state::<Arc<Mutex<BackendGlobalState>>>();
    let mut state = match state.lock() {
        Ok(state) => state,
        Err(err) => { log_lock_error_void(format!("Lock error when attempting to auto_connect_to_app: {err}"), app_handle); return; }, // Early return
    };
    let network_info = &mut state.network_info;

    if let Some(app) = network_info.discovered_apps.get_mut(connect_to_id) {
        if !app.info.online || (app.info.authorized_by_self && app.info.authorized_by_peer) {
            return; // Only attempt auto connection for an online app that is not connected
        }
        // TODO, maybe reset authorized_by_self=false if not online and app.info.authorized_by_peer=false
        let content = ConnectRequestContent {
            password: app.info.password.clone(),
        };
        drop(state);
        send_connect_to_app(connect_to_id, content, app_handle);

        let message = format!("Attempting to connect automatically to {}", connect_to_id);
        log::info!("{}", message);
        backend_add_log(message, LogLevel::Info, app_handle);
    }
}

/// If content is None, then no request is sent to peer.
pub fn send_disconnect_from_app(
    disconnect_from_id: &String,
    content: Option<DisconnectRequestContent>,
    network_info: &mut NetworkInfo,
    app_handle: &AppHandle
) {
    if let Some(content) = content {
        let request_content = match postcard::to_allocvec(&content) {
            Ok(request_content) => request_content,
            Err(_) => { // Should never happen
                return; // Early return
            },
        };
        let _ = submit_network_request(NetworkApplicationRequest {
            to_id: disconnect_from_id.clone(),
            action: NetworkAction::Disconnect,
            content: request_content,
        }, network_info);
    }

    if let Some(app) = network_info.discovered_apps.get_mut(disconnect_from_id) {
        app.info.authorized_by_self = false;
        app.info.authorized_by_peer = false;
    }

    // Set focus back to self, if the focus was on the disconnected app
    if network_info.self_info.focused_id == *disconnect_from_id {
        network_info.self_info.focused_id = network_info.self_info.id.clone();
    }

    // Forgetting an app is done outside this function, for manual disconnection.

    let self_response: AppResponse = AppResponse::from(&network_info.self_info);
    let response: Vec<AppResponse> = network_info.discovered_apps.values().map(|app|
        AppResponse::from(&app.info)).collect();
    to_frontend_update_self_app(self_response, app_handle);
    to_frontend_update_discovered_apps(response, app_handle);
}

/// Returns true if the app was added
pub fn add_app_if_missing(
    app_id: &String,
    network_info: &mut NetworkInfo,
    config: HardDiskStorage,
    app_handle: &AppHandle
) -> bool {
    if network_info.discovered_apps.contains_key(app_id) {
        return false; // Early return
    }

    // Some default values can be derived from config
    let mut app_name = "Other app".to_string();
    let mut password = "".to_string();
    if let Some(stored_app) = config.other_apps.iter().find(|app| app.id == *app_id) {
        password = stored_app.password.clone();
        app_name = stored_app.app_name.clone();
    }

    let result = network_info.discovered_apps.insert(app_id.clone(), OtherAppInfo {
        info : AppInfo {
            name: app_name,
            password,
            id: app_id.clone(),
            address_infos: vec!(),
            set_of_monitors: SetOfMonitors {
                offset_x: 0,
                offset_y: 0,
                monitors: vec!(),
            },
            online: true,
            focused_id: "".to_string(), // Focus of another app is unused
            connected_ids: vec!(),
            authorized_by_self: false,
            authorized_by_peer: false,
            file_transfers: VecDeque::from(vec!()),
        },
        requests_queue: VecDeque::from(vec!()),
        responses_queue: VecDeque::from(vec!()),
        received_requests_queue: VecDeque::from(vec!()),
        received_file_chunks: std::collections::HashMap::new(),
        confirmed_file_chunks: std::collections::HashMap::new(),
        consecutive_failed_requests: 0,
    });

    let response: Vec<AppResponse> = network_info.discovered_apps.values().map(|app|
        AppResponse::from(&app.info)).collect();
    to_frontend_update_discovered_apps(response, app_handle);

    result.is_none()
}