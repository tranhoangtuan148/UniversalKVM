use crate::{PREVENT_TAURI_CRASH, get_handle, return_back_handle};
use crate::common::{backend_add_log, log_lock_error, log_lock_error_void, to_frontend_update_borders, to_frontend_update_mouse_devices};
use crate::device_names::{DeviceKind, friendly_device_name};
use crate::focus::send_focus_with_border;
use crate::networking::{submit_network_request};
use crate::states::{ActiveMouseBackendResponse, AppSetOfMonitors, BackendGlobalState, Border, BorderPair, BorderPortal, BordersResponse, LogLevel, Monitor, MouseEventRequestContent, MouseInfo, NetworkAction, NetworkApplicationRequest, NetworkInfo, RememberedDevice, SetOfMonitors};
use crate::storage::save_config;

use std::sync::{Arc, Mutex};

use xavkeyboardandmousegrabber::{MouseProperties, mouse_events};
use tauri::{AppHandle, Manager};

// Returns true if a mouse has been added or removed
pub fn discover_available_mouses(app_handle: &AppHandle) -> Result<bool, String> {
    let mut has_been_updated = false;

    let mut available_mouses = xavkeyboardandmousegrabber::list_available_mouses();
    /*
      Windows names a mouse after its driver class, so every device came back as
      "HID-compliant mouse". A mouse is keyed on its name and its path, so the resolved
      name has to replace the reported one on both the listing and the opened device,
      otherwise the two would no longer agree on a key.
    */
    for mouse_properties in &mut available_mouses {
        mouse_properties.device_name = friendly_device_name(
            &mouse_properties.device_path, &mouse_properties.device_name, DeviceKind::Mouse);
    }

    let state = app_handle.state::<Arc<Mutex<BackendGlobalState>>>();
    let mut state = match state.lock() {
        Ok(state) => state,
        Err(err) => return log_lock_error(format!("Lock error when attempting to get available mouses: {err}"), app_handle), // Early return
    };
    let remembered_mouses = state.hard_disk_storage.remembered_mouses.clone();
    let mouses= &mut state.mouses_info_map;

    // Add new available mouses
    for mouse_properties in &available_mouses {
        let mut is_physical_mouse = false;
        if let Some(mouse) = mouses.get(&mouse_properties.get_key()) {
            is_physical_mouse = mouse.mouse.is_real_device();
        }
        if is_physical_mouse {
            // Nothing to do
        } else {
            let mouse_result = xavkeyboardandmousegrabber::get_mouse(mouse_properties.device_path.to_string(), false);
            match mouse_result {
                Ok(mut mouse) => {
                    mouse.device_name = friendly_device_name(
                        &mouse.device_path, &mouse.device_name, DeviceKind::Mouse);
                    // Remembered mice are matched on their path alone, because a resolved
                    // name can change between versions of this app, a path cannot.
                    let default_active: bool = remembered_mouses.iter().any(|remembered_mouse|
                        remembered_mouse.id == mouse.device_path);
                    mouses.insert(mouse.get_key(), MouseInfo {
                        mouse,
                        active: default_active,
                        virtual_mouse: None,
                    });
                    has_been_updated = true;
                },
                Err(error) => {
                    log::error!("Error when reading mouse {} for path {} : {}", mouse_properties.device_name, mouse_properties.device_path, error);
                    backend_add_log(
                        format!("Error when reading mouse {} for path {} : {}", mouse_properties.device_name, mouse_properties.device_path, error),
                        LogLevel::Error,
                        app_handle
                    );
                },
            }
        }
    }

    // Free mouses no longer available
    for (key, mouse_info) in &mut *mouses {
        if !available_mouses.iter().any(|available_mouse| available_mouse.get_key() == *key) {
            has_been_updated = true;
            if mouse_info.mouse.is_grabbed() {
                let _ = mouse_info.mouse.ungrab();
            }
        }
    }
    // Remove physical mouse no longer available
    mouses.retain(|key, mouse|
        available_mouses.iter().any(|available_mouse| available_mouse.get_key() == *key)
            || mouse.virtual_mouse.is_some());


    // Before returning, update the mouses on the frontend
    let response: Vec<ActiveMouseBackendResponse> = mouses.values().map(|mouse_info| ActiveMouseBackendResponse {
        name: mouse_info.mouse.device_name.to_string(),
        id: mouse_info.mouse.device_path.to_string(),
        active: mouse_info.active,
    }).collect();
    drop(state); // For optimisation
    to_frontend_update_mouse_devices(response, app_handle);

    Ok(has_been_updated)
}

// Returns true if a mouse has been activated or inactivated
pub fn update_active_mouses(updated_mouses: Vec<ActiveMouseBackendResponse>, app_handle: &AppHandle) -> Result<bool, String> {
    let mut has_been_updated = false;

    let state = app_handle.state::<Arc<Mutex<BackendGlobalState>>>();
    let mut state = match state.lock() {
        Ok(state) => state,
        Err(err) => return log_lock_error(format!("Lock error when attempting to update active mouses: {err}"), app_handle), // Early return
    };
    let mouses= &mut state.mouses_info_map;

    let mut mouses_to_remember = vec!();
    let mut mouses_to_forget = vec!();

    // Update mouses that have a difference in the active status
    for mouse_info in (*mouses).values_mut() {
        let updated_mouse = match updated_mouses.iter().find(|mouse|
            mouse.id == mouse_info.mouse.device_path && mouse.name == mouse_info.mouse.device_name
        ) {
            Some(updated_mouse) => updated_mouse,
            None => continue, // Nothing to update, this would be an invalid mouse
        };
        if mouse_info.active != updated_mouse.active {
            has_been_updated = true;
            mouse_info.active = updated_mouse.active;
            if updated_mouse.active {
                mouses_to_remember.push(RememberedDevice {
                    id: updated_mouse.id.clone(),
                    name: updated_mouse.name.clone(),
                });
            } else {
                mouses_to_forget.push(RememberedDevice {
                    id: updated_mouse.id.clone(),
                    name: updated_mouse.name.clone(),
                });
            }
        }
    }

    // Update configuration to remember user choices. If a choice is not explicitly removed by the user, it is remembered.
    // Matched on the id, which is the device path, so a device the user chose keeps being
    // remembered when its resolved name changes.
    state.hard_disk_storage.remembered_mouses.retain(
        |mouse| !mouses_to_forget.iter().any(|mouse_to_forget| mouse_to_forget.id == mouse.id));
    for mouse_to_remember in mouses_to_remember {
        state.hard_disk_storage.remembered_mouses.retain(
            |remembered_mouse| remembered_mouse.id != mouse_to_remember.id);
        state.hard_disk_storage.remembered_mouses.push(mouse_to_remember);
    }
    let config = state.hard_disk_storage.clone();
    let _ = save_config(app_handle, config);

    Ok(has_been_updated)
}

pub fn fetch_mouse_events(app_handle: &AppHandle) {
    let state = app_handle.state::<Arc<Mutex<BackendGlobalState>>>();
    let mut state = match state.lock() {
        Ok(state) => state,
        Err(err) => { log_lock_error_void(format!("Lock error when fetching mouse events: {err}"), app_handle); return; }, // Early return
    };
    let peer = state.network_info.discovered_apps.iter().find(|(_key, app)|
        app.info.authorized_by_self && app.info.authorized_by_peer
        && state.network_info.self_info.focused_id == app.info.id);
    let focused_peer_id = match peer {
        Some(app) => app.1.info.id.clone(),
        None => "".to_string(),
    };

    let mouses = &mut (state.mouses_info_map);

    let mut requests_to_send: Vec<NetworkApplicationRequest> = vec!();
    let mut has_received_an_event = false;
    for mouse_info in (*mouses).values_mut() {
        if !mouse_info.mouse.is_real_device() {
            continue; // No events to fetch for a virtual only mouse
        }
        /*
          A mouse that replays a peer's events is not a mouse of this machine. It carries
          the peer's device path, which on Windows is enough for is_real_device to call it
          real, and grabbing on Windows blocks every mouse of the machine, so capturing it
          would block the mice the user actually holds.
        */
        if mouse_info.virtual_mouse.is_some() {
            continue;
        }
        if !mouse_info.active {
            if mouse_info.mouse.is_grabbed() {
                let _ = mouse_info.mouse.ungrab();
            }
            continue; // Don't send events for inactive mouses
        }

        // Grab or ungrab mouse depending on focus
        if !focused_peer_id.is_empty() {
            if !mouse_info.mouse.is_grabbed() {
                let _ = mouse_info.mouse.grab();
            }
        } else if mouse_info.mouse.is_grabbed() {
            let _ = mouse_info.mouse.ungrab();
        }

        match mouse_info.mouse.get_recent_events() {
            Ok(events) => {
                /*
                  Only a real event counts. On Windows and macOS an idle device answers with
                  an empty list rather than an error, and taking that for an event had this
                  loop ask for a border check every millisecond, forever.
                */
                if !events.is_empty() {
                    has_received_an_event = true;
                }

                // Nothing to do if focus is already on the current app
                if focused_peer_id.is_empty() {
                    continue;
                }

                // Optimization: no need to send a request when there is no events
                if events.is_empty() {
                    continue;
                }

                // Network optimization: combine relative mouse movements.
                // This optimization is significant, because mouse movement can happen frequently, e.g. every 1ms.
                let mut combined_events = vec!();
                for mut event in events {
                    if let xavkeyboardandmousegrabber::MouseEvent::MovementEvent(movement_event) = &mut event {
                        // Erase event name as an optimization
                        movement_event.name = "".to_string();

                        let existing_event = combined_events.iter_mut().find(|event| {
                            if let xavkeyboardandmousegrabber::MouseEvent::MovementEvent(existing_movement_event) = event {
                                return existing_movement_event.movement_type == movement_event.movement_type
                                    && movement_event.movement_type == xavkeyboardandmousegrabber::MouseMovementType::RELATIVE as u16;
                            }
                            false
                        });
                        if let Some(existing_event) = existing_event {
                            // This if should always be true, used to cast MouseEvent to MouseMovementEvent
                            if let xavkeyboardandmousegrabber::MouseEvent::MovementEvent(existing_movement_event) = existing_event {
                                existing_movement_event.x += movement_event.x;
                                existing_movement_event.y += movement_event.y;
                            }
                            continue;
                        }
                    }
                    combined_events.push(event);
                }


                // For simplicity, supported buttons are generated automatically on the app creating a virtual device
                let mut mouse_properties = mouse_info.mouse.get_mouse_properties_without_keys();
                let virtual_mouse_name = format!("{} Virtual Mouse", mouse_properties.device_name);
                mouse_properties.device_name = virtual_mouse_name;
                let content = MouseEventRequestContent {
                    events: combined_events,
                    mouse_properties,
                };
                let request_content = match postcard::to_allocvec(&content) {
                    Ok(request_content) => request_content,
                    Err(_) => { // Should never happen
                        return; // Early return
                    },
                };

                requests_to_send.push(NetworkApplicationRequest {
                    to_id: focused_peer_id.clone(),
                    action: NetworkAction::MouseEvent,
                    content: request_content,
                });
            }
            Err(err) => {
                let error = format!("Error when reading events for {}: {}", mouse_info.mouse.device_name, err);
                log::debug!("{}", error.to_string());
                // Not logged, because this error is very common.
                // backend_add_log(error.to_string(), &app_handle);
            }
        }
    }

    // If a mouse event was received, check if the cursor is on a border
    // Applies only if the focus is on onself.
    if focused_peer_id.is_empty() && has_received_an_event {
        send_focus_if_cursor_is_on_valid_border();
    }

    for request in requests_to_send {
        submit_network_request(request, &mut state.network_info);
    }
}

pub fn execute_mouse_events(app_handle: &AppHandle) {
    let state = app_handle.state::<Arc<Mutex<BackendGlobalState>>>();
    let mut state = match state.lock() {
        Ok(state) => state,
        Err(err) => { log_lock_error_void(format!("Lock error when executing mouse events: {err}"), app_handle); return; }, // Early return
    };
    let mouses_events = state.received_mouses_events_queue.clone();
    state.received_mouses_events_queue.clear();
    drop(state); // Necessary to prevent a deadlock

    let has_events = !mouses_events.is_empty();
    for mouse_event in mouses_events {
        let _ = send_events_to_mouse(mouse_event.events, mouse_event.mouse_properties, app_handle);
    }

    // If a mouse event was executed, check if the cursor is on a border
    if has_events {
        send_focus_if_cursor_is_on_valid_border();
    }
}

pub fn send_events_to_mouse(events: Vec<mouse_events::MouseEvent>, mut mouse_properties: MouseProperties, app_handle: &AppHandle) -> Result<(), String> {
    let state = app_handle.state::<Arc<Mutex<BackendGlobalState>>>();
    let mut state = match state.lock() {
        Ok(state) => state,
        Err(err) => return log_lock_error(format!("Lock error when attempting to send events to mouses: {err}"), app_handle), // Early return
    };
    let mouses= &mut state.mouses_info_map;

    // If device is a virtual mouse from another machine, it needs to be created.
    if mouses.get(&mouse_properties.get_key()).is_none() {
        if mouse_properties.supported_keys.is_empty() {
            mouse_properties.supported_keys = xavkeyboardandmousegrabber::mouse_events::get_default_supported_mouse_buttons();
        }

        let virtual_mouse_info = MouseInfo {
            mouse: xavkeyboardandmousegrabber::Mouse::new_uninitialized(&mouse_properties),
            active: true,
            virtual_mouse: None, // Will be created in the code below
        };
        mouses.insert(mouse_properties.get_key(), virtual_mouse_info);
    }

    if let Some(mouse_info) = mouses.get_mut(&mouse_properties.get_key()) {
        if mouse_info.virtual_mouse.is_none() {
            let virtual_mouse_result = xavkeyboardandmousegrabber::VirtualMouseBuilder::new()
                .delay_ms(0)
                .name(mouse_properties.device_name.to_string())
                .set_supported_keys(&mouse_properties.supported_keys)
                .build();

            match virtual_mouse_result {
                Ok(virtual_mouse) => {
                    mouse_info.virtual_mouse = Some(virtual_mouse);
                },
                Err(err) => {
                    let error = format!("Error when creating {}: {}", mouse_properties.device_name, err);
                    log::error!("{}", error.to_string());
                    backend_add_log(error.to_string(), LogLevel::Error, app_handle);
                    return Err(error); // Early return
                },
            }
        }

        if let Some(virtual_mouse) = &mut mouse_info.virtual_mouse {
            for event in events {
                match event {
                    mouse_events::MouseEvent::KeyEvent(event) => {
                        match virtual_mouse.send_button_event(event.code, event.state.clone()) {
                            Ok(_) => (),
                            Err(err) => {
                                let error = format!("Error when sending key {} {:?} to {}: {}", event.code, event.state, mouse_properties.device_name, err);
                                log::error!("{}", error.to_string());
                                backend_add_log(error.to_string(), LogLevel::Error, app_handle);
                                return Err(error); // Early return
                            },
                        }
                    },
                    mouse_events::MouseEvent::MovementEvent(event) => {
                        if event.movement_type == mouse_events::MouseMovementType::ABSOLUTE as u16 {
                            // TODO, need to be able to send to 1 axis at a time
                            match virtual_mouse.move_cursor_absolute(mouse_events::MouseMovement {
                                x: event.x,
                                y: event.y,
                            }) {
                                Ok(_) => (),
                                Err(err) => {
                                    let error = format!("Error when positioning mouse with {}: {}", mouse_properties.device_name, err);
                                    log::error!("{}", error.to_string());
                                    // Not logged on the frontend to avoid flooding. User will notice the bug anyway.
                                    return Err(error); // Early return
                                },
                            }
                        } else {
                            match virtual_mouse.move_cursor(mouse_events::MouseMovement {
                                x: event.x,
                                y: event.y,
                            }) {
                                Ok(_) => (),
                                Err(err) => {
                                    let error = format!("Error when moving mouse with {}: {}", mouse_properties.device_name, err);
                                    log::error!("{}", error.to_string());
                                    // Not logged on the frontend to avoid flooding. User will notice the bug anyway.
                                    return Err(error); // Early return
                                },
                            }
                        }
                    },
                    mouse_events::MouseEvent::WheelEvent(event) => {
                        match virtual_mouse.move_wheel(mouse_events::WheelMovement {
                            horizontal: event.horizontal,
                            vertical: event.vertical,
                            is_high_resolution: event.is_high_resolution,
                        }) {
                            Ok(_) => (),
                            Err(err) => {
                                let error = format!("Error when sending wheel event with {}: {}", mouse_properties.device_name, err);
                                log::error!("{}", error.to_string());
                                backend_add_log(error.to_string(), LogLevel::Error, app_handle);
                                return Err(error); // Early return
                            },
                        }
                    },
                }
            }
        }
    }

    Ok(())
}

/*
  The border check is expensive and it is asked for constantly: once per pass of the fetch
  loop while a mouse moves, and once per batch of events received from a peer. Reading the
  cursor position goes to the Tauri event loop and waits for the answer, and the check holds
  the global state while it works, so running it at loop rate makes the machine being driven
  apply the movement it receives in fits.

  So the check is bounded twice: one at a time, and not more often than every few
  milliseconds. A crossing is not missed by waiting: the cursor stays on the border until
  the check pushes it away.
*/
static IS_CHECKING_BORDER: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
static LAST_BORDER_CHECK_MS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
const BORDER_CHECK_INTERVAL_MS: u64 = 10;

/// Releases the gate whatever happens to the check, so a failed one cannot end crossings.
struct BorderCheckGate;
impl Drop for BorderCheckGate {
    fn drop(&mut self) {
        IS_CHECKING_BORDER.store(false, std::sync::atomic::Ordering::SeqCst);
    }
}

fn now_ms() -> u64 {
    let since_the_epoch = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    since_the_epoch.as_millis() as u64
}

pub fn send_focus_if_cursor_is_on_valid_border() {
    let now = now_ms();
    if now.saturating_sub(LAST_BORDER_CHECK_MS.load(std::sync::atomic::Ordering::SeqCst)) < BORDER_CHECK_INTERVAL_MS {
        return; // Early return, the cursor was where it is a moment ago
    }
    if IS_CHECKING_BORDER.swap(true, std::sync::atomic::Ordering::SeqCst) {
        return; // Early return, a check is already reading the cursor position
    }
    LAST_BORDER_CHECK_MS.store(now, std::sync::atomic::Ordering::SeqCst);

    let app_handle = get_handle();

    // Using a thread because get_cursor_position and repulse_cursor_from_border can block when the UI is frozen
    std::thread::spawn(move || {
        let _gate = BorderCheckGate;
        send_focus_if_cursor_is_on_valid_border_blocking(&app_handle);
        return_back_handle(app_handle);
    });
}

/// Blocks on the cursor position, so it belongs on the thread that
/// `send_focus_if_cursor_is_on_valid_border` spawns, never on a fetch loop.
fn send_focus_if_cursor_is_on_valid_border_blocking(app_handle: &AppHandle) {
    let cursor = get_cursor_position(app_handle);
    let state = app_handle.state::<Arc<Mutex<BackendGlobalState>>>();
    let mut state = match state.lock() {
        Ok(state) => state,
        Err(err) => { log_lock_error_void(format!("Lock error when send_focus_if_cursor_is_on_valid_border: {err}"), app_handle); return; }, // Early return
    };
    let self_monitors = &state.network_info.self_info.set_of_monitors.monitors;

    if let Some(cursor) = cursor {
        let on_global_border = is_cursor_on_global_border(
            cursor.clone(),
            self_monitors,
            &state.network_info.self_info.id,
            &state.network_info.borders
        );

        if let Some(border_portal) = on_global_border {
            let focused_id = border_portal.linked_app_id.clone();

            // Only send focus if connected with the other app
            if let Some(app) = state.network_info.discovered_apps.get(&focused_id)
                && app.info.authorized_by_self && app.info.authorized_by_peer {
                send_focus_with_border(focused_id, Some(border_portal.clone()), &mut state.network_info, app_handle);
                drop(state); // Drop state, because repulse_cursor_from_border can block
                repulse_cursor_from_border(cursor, border_portal, app_handle);
            }
        }
    }
}

/// Get x and y position in pixels, on the global area (all monitors combined).
///
/// Warning: if the UI is frozen, e.g. when a user minimizes the app on Windows, the function blocks.
///     Therefore, this function should not be called when the global state is locked.
pub fn get_cursor_position(app_handle: &AppHandle) -> Option<xavkeyboardandmousegrabber::MouseMovement> {
    let prevent_tauri_crash = PREVENT_TAURI_CRASH.get_or_init(|| Mutex::new(()));
    match prevent_tauri_crash.lock() {
        Ok(_) => {
            let cursor = app_handle.cursor_position();
            match cursor {
                Ok(cursor) => Some(xavkeyboardandmousegrabber::MouseMovement {
                    x: cursor.x.round() as i32,
                    y: cursor.y.round() as i32,
                }),
                Err(_) => None,
            }
        },
        Err(_) => None,
    }
}

/// Set x and y position in pixels, on the global area (all monitors combined).
///
/// Warning: if the UI is frozen, e.g. when a user minimizes the app on Windows, the function blocks.
///     Therefore, this function should not be called when the global state is locked.
pub fn set_cursor_position(x: i32, y: i32, app_handle: &AppHandle) -> Result<(), String> {
    let prevent_tauri_crash = PREVENT_TAURI_CRASH.get_or_init(|| Mutex::new(()));
    match prevent_tauri_crash.lock() {
        Ok(_) => {
            let mut position = tauri::PhysicalPosition { x, y };
            let window = app_handle.get_webview_window("main");
            if let Some(window) = window {
                let window_position = match window.inner_position() {
                    Ok(position) => position,
                    Err(err) => { return Err(format!("Could not get window position: {}", err)) },
                };
                // Convert x and y into window relative position
                position.x -= window_position.x;
                position.y -= window_position.y;
                let result = window.set_cursor_position(position);
                return match result {
                    Ok(_) => Ok(()),
                    Err(err) => Err(format!("Failed to move cursor: {}", err).to_string()),
                };
            }
            Err("No window found".to_string())
        },
        Err(_) => Err("Failed to set cursor because of lock".to_string()),
    }
}

/// Returns a current list of monitors detected by Tauri
///
/// Warning: if the UI is frozen, e.g. when a user minimizes the app on Windows, the function blocks.
///     Therefore, this function should not be called when the global state is locked.
pub fn fetch_self_monitors(app_handle: &AppHandle) -> Vec<Monitor> {
    let prevent_tauri_crash = PREVENT_TAURI_CRASH.get_or_init(|| Mutex::new(()));
    let monitors = match prevent_tauri_crash.lock() {
        Ok(_) => {
            app_handle.available_monitors()
        },
        Err(err) => {
            log::error!("Failed to get monitors because of lock: {}", err);
            return vec!();
        },
    };

    match monitors {
        Ok(monitors) => {
            let mut result = Vec::with_capacity(monitors.len());
            for monitor in &monitors {
                let position = monitor.position();
                let size = monitor.size();
                result.push(Monitor {
                    x: position.x,
                    y: position.y,
                    width: size.width as u16,
                    height: size.height as u16,
                    // By default, a monitor has a gray color
                    color_r: 127,
                    color_g: 127,
                    color_b: 127,
                });
            }
            Monitor::sort_monitors(&mut result);
            result
        },
        Err(err) => {
            log::error!("Failed to get monitors: {}", err);
            vec!()
        },
    }
}

/// Returns a list of monitors for each app, including self and discovered apps
pub fn get_all_apps_monitors(network_info: &NetworkInfo) -> Vec<AppSetOfMonitors> {
    let mut result = Vec::with_capacity(1 + network_info.discovered_apps.len());
    result.push(AppSetOfMonitors {
        id: network_info.self_info.id.clone(),
        set_of_monitors: network_info.self_info.set_of_monitors.clone(),
    });
    for app in network_info.discovered_apps.values() {
        result.push(AppSetOfMonitors {
            id: app.info.id.clone(),
            set_of_monitors: app.info.set_of_monitors.clone(),
        });
    }
    result
}

pub fn update_all_apps_monitors(apps_monitors: Vec<AppSetOfMonitors>, network_info: &mut NetworkInfo) {
    for app_monitors in apps_monitors {
        if app_monitors.id == network_info.self_info.id {
            network_info.self_info.set_of_monitors = app_monitors.set_of_monitors;
        } else if let Some(app) = network_info.discovered_apps.get_mut(&app_monitors.id) {
            app.info.set_of_monitors = app_monitors.set_of_monitors;
        }
    }
}

/// This function removes existing borders that are related to changed monitors.
/// It also updates and save data related to borders.
pub fn apply_new_borders(
    state: &mut BackendGlobalState,
    new_borders: Vec<BorderPair>,
    apps_monitors: Vec<AppSetOfMonitors>,
    app_handle: &AppHandle
) {
    state.network_info.borders = update_borders(&state.network_info.borders, new_borders, &apps_monitors);
    update_all_apps_monitors(apps_monitors, &mut state.network_info);
    let mut config = state.hard_disk_storage.clone();
    config.monitor_offset_x = state.network_info.self_info.set_of_monitors.offset_x;
    config.monitor_offset_y = state.network_info.self_info.set_of_monitors.offset_y;
    config.update_borders(&state.network_info);
    state.hard_disk_storage = config.clone();
    let _ = save_config(app_handle, config);
    let borders_response = BordersResponse { borders: state.network_info.borders.clone() };
    to_frontend_update_borders(borders_response, app_handle);
}

pub fn update_borders(previous_borders: &[BorderPair], new_borders: Vec<BorderPair>, apps_monitors: &[AppSetOfMonitors]) -> Vec<BorderPair> {
    let mut updated_borders = vec!();
    
    // Preserve borders unrelated to new borders and modified apps
    for border in previous_borders {
        if !border.is_subset(apps_monitors) {
            updated_borders.push(border.clone());
        }
    }

    updated_borders.extend(new_borders);

    updated_borders
}

/// Returns true if a monitor was removed or added.
pub fn update_set_of_monitors(set_of_monitors: &mut SetOfMonitors, current_monitors: Vec<Monitor>) -> bool {
    let mut monitors_to_add = vec!();
    let mut result = false;

    let prev_length = set_of_monitors.monitors.len();
    // Remove monitors that don't match the current monitors
    set_of_monitors.monitors.retain(|monitor| current_monitors.iter().any(|current_monitor|
        monitor.is_physically_identical(current_monitor)));
    result = result || prev_length != set_of_monitors.monitors.len();

    for current_monitor in current_monitors {
        let existing_monitor = set_of_monitors.monitors.iter().find(|monitor|
            monitor.is_physically_identical(&current_monitor));
        if existing_monitor.is_none() {
            monitors_to_add.push(current_monitor);
        }
    }
    result = result || !monitors_to_add.is_empty();

    while !monitors_to_add.is_empty() {
        let new_monitor = monitors_to_add.pop();
        if let Some(new_monitor) = new_monitor {
            set_of_monitors.monitors.push(new_monitor);
        }
    }

    Monitor::sort_monitors(&mut set_of_monitors.monitors);
    result
}

#[cfg(target_os = "macos")]
const OUTSIDE_TOLERANCE: i32 = 300; // On macOS, cursor can go out of bound.
#[cfg(not(target_os = "macos"))]
const OUTSIDE_TOLERANCE: i32 = 0;

pub fn is_cursor_in_monitor(cursor_x: i32, cursor_y: i32, monitor: &Monitor) -> bool {
    cursor_x >= monitor.x - OUTSIDE_TOLERANCE && cursor_x < monitor.x + monitor.width as i32 + OUTSIDE_TOLERANCE
        && cursor_y >= monitor.y - OUTSIDE_TOLERANCE && cursor_y < monitor.y + monitor.height as i32 + OUTSIDE_TOLERANCE

}

/// Position `x` and `y` are in pixels, on the global area (all monitors combined).
///
/// If a cursor is on the monitor corner, the left or right border will be returned.
pub fn is_cursor_on_border(cursor_x: i32, cursor_y: i32, monitor: &Monitor) -> Option<Border> {
    const TOLERANCE: i32 = 1; // Consider the cursor on a border within a tolerance.

    let is_cursor_in_monitor = is_cursor_in_monitor(cursor_x, cursor_y, monitor);

    if is_cursor_in_monitor
        && cursor_x >= monitor.x - OUTSIDE_TOLERANCE
        && cursor_x <= monitor.x + TOLERANCE {
        return Some(Border::Left);
    }
    if is_cursor_in_monitor
        && cursor_x >= monitor.x + monitor.width as i32 - TOLERANCE
        && cursor_x <= monitor.x + monitor.width as i32 + OUTSIDE_TOLERANCE {
        return Some(Border::Right);
    }
    if is_cursor_in_monitor
        && cursor_y >= monitor.y - OUTSIDE_TOLERANCE
        && cursor_y <= monitor.y + TOLERANCE {
        return Some(Border::Top);
    }
    if is_cursor_in_monitor
        && cursor_y >= monitor.y + monitor.height as i32 - TOLERANCE
        && cursor_y <= monitor.y + monitor.height as i32 + OUTSIDE_TOLERANCE {
        return Some(Border::Bottom);
    }
    None
}

/// Returns a border portal if the cursor is on the global border of the current app, that is a border that has no other monitor next to it.
///
/// Example with two monitors (B = valid global border, n = not a global border):
///
/// ```text
/// BBBBBBBBBBBBB BBBBBBBBBBBBBBB
/// B monitor 1 n n  monitor 2  B
/// B           n nBBBBBBBBBBBBBB
/// BBBBBBBBBBBBB
/// ```
///
pub fn is_cursor_on_global_border(cursor: xavkeyboardandmousegrabber::MouseMovement, self_monitors: &Vec<Monitor>, self_id: &str, borders: &[BorderPair]) -> Option<BorderPortal> {
    let mut on_border = None;
    let mut on_monitor = None;
    let mut monitor_index: u8 = 0;
    for (index, monitor) in self_monitors.iter().enumerate() {
        if let Some(border) = is_cursor_on_border(cursor.x, cursor.y, monitor) {
            on_border = Some(border);
            on_monitor = Some(monitor);
            monitor_index = index as u8;
            break;
        }
    }

    let border = match on_border {
        Some(border) => border,
        None => { return None; }, // Early return
    };
    let on_monitor = match on_monitor {
        Some(monitor) => monitor,
        None => { return None; }, // Should never happen
    };

    // Verify that the border is on the global border
    for monitor in self_monitors {
        // No check on the same monitor
        if std::ptr::eq(monitor, on_monitor) {
            continue;
        }

        match border {
            Border::Left => {
                // Returns None if another border on the left is detected
                if monitor.x < cursor.x
                    && monitor.y <= cursor.y && cursor.y <= monitor.y + monitor.height as i32 {
                    return None; // Early return
                }
            },
            Border::Right => {
                // Returns None if another border on the right is detected
                if cursor.x < monitor.x + monitor.width as i32
                    && monitor.y <= cursor.y && cursor.y <= monitor.y + monitor.height as i32 {
                    return None; // Early return
                }
            },
            Border::Top => {
                // Returns None if another border on the top is detected
                if monitor.y < cursor.y
                    && monitor.x <= cursor.x && cursor.x <= monitor.x + monitor.width as i32 {
                    return None; // Early return
                }
            },
            Border::Bottom => {
                // Returns None if another border on the bottom is detected
                if cursor.y < monitor.y + monitor.height as i32
                    && monitor.x <= cursor.x && cursor.x <= monitor.x + monitor.width as i32 {
                    return None; // Early return
                }
            },
        }
    }

    let monitor_borders = Monitor::get_monitor_borders(self_id, monitor_index, borders);
    for monitor_border in &monitor_borders {
        let mut self_border = &monitor_border.pair[0];
        let mut other_border = &monitor_border.pair[1];
        if self_border.app_id != self_id {
            self_border = &monitor_border.pair[1];
            other_border = &monitor_border.pair[0];
        }
        if border.clone() as u8 != self_border.border {
            continue;
        }

        let start = self_border.start as f32;
        let mut end = self_border.end as f32;
        if end - start < 1.0 {
            end = start + 1.0; // prevent division by 0
        }

        let cursor_position = match border {
            Border::Left => cursor.y as f32 - on_monitor.y as f32,
            Border::Right => cursor.y as f32 - on_monitor.y as f32,
            Border::Top => cursor.x as f32 - on_monitor.x as f32,
            Border::Bottom => cursor.x as f32 - on_monitor.x as f32,
        };

        if start <= cursor_position && cursor_position <= end {
            return Some(BorderPortal {
                position: (cursor_position - start) / (end - start),
                border: self_border.border,

                linked_border: other_border.border,
                linked_monitor_index: other_border.monitor_index,
                linked_start: other_border.start,
                linked_end: other_border.end,
                linked_app_id: other_border.app_id.clone(),
            });
        }
    }

    None
}

/// Returns a position to teleport to, if any
pub fn get_position_from_border_portal(border_portal: BorderPortal, self_monitors: &[Monitor]) -> Option<xavkeyboardandmousegrabber::MouseMovement> {
    let offset = 5; // To prevent teleport back

    if border_portal.linked_monitor_index >= self_monitors.len() as u8 {
        return None;
    }
    let monitor: &Monitor = &self_monitors[border_portal.linked_monitor_index as usize];

    let mut scale = (border_portal.linked_end - border_portal.linked_start) as f32;
    if scale < 1.0 {
        scale = 1.0;
    }

    let mut position = xavkeyboardandmousegrabber::MouseMovement {
        x: monitor.x,
        y: monitor.y
    };
    match border_portal.linked_border {
        val if val == Border::Left as u8 => {
            position.x += offset;
            position.y += border_portal.linked_start + (scale * border_portal.position).round() as i32;
        },
        val if val == Border::Right as u8 => {
            position.x += monitor.width as i32 - offset;
            position.y += border_portal.linked_start + (scale * border_portal.position).round() as i32;
        },
        val if val == Border::Top as u8 => {
            position.x += border_portal.linked_start + (scale * border_portal.position).round() as i32;
            position.y += offset;
        },
        _ /* Bottom */ => {
            position.x += border_portal.linked_start + (scale * border_portal.position).round() as i32;
            position.y += monitor.height as i32 - offset;
        },
    };

    Some(position)
}

/// Push back the cursor from a border, to prevent having a cursor hanging near a border, which is
/// undesirable and can cause unstable behavior when focus alternates quickly between two monitors.
///
/// Warning: if the UI is frozen, this function can block because of set_cursor_position
pub fn repulse_cursor_from_border(cursor: xavkeyboardandmousegrabber::MouseMovement, border_portal: BorderPortal, app_handle: &AppHandle) {
    let offset = 5;

    match border_portal.border {
        val if val == Border::Left as u8 => {
            let _ = set_cursor_position(cursor.x + offset, cursor.y, app_handle);
        },
        val if val == Border::Right as u8 => {
            let _ = set_cursor_position(cursor.x - offset, cursor.y, app_handle);
        },
        val if val == Border::Top as u8 => {
            let _ = set_cursor_position(cursor.x, cursor.y + offset, app_handle);
        },
        _ /* Bottom */ => {
            let _ = set_cursor_position(cursor.x, cursor.y - offset, app_handle);
        },
    }

}