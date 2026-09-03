use crate::common::{backend_add_log, log_lock_error, log_lock_error_void, to_frontend_update_keyboard_devices};
use crate::device_names::{DeviceKind, friendly_device_name};
use crate::networking::{submit_network_request};
use crate::states::{ActiveKeyboardBackendResponse, BackendGlobalState, KeyboardEventRequestContent, KeyboardInfo, LogLevel, NetworkAction, NetworkApplicationRequest, RememberedDevice};
use crate::storage::save_config;

use std::sync::{Arc, Mutex};

use xavkeyboardandmousegrabber::{KeyboardProperties, key_events};
use tauri::{AppHandle, Manager};

// Returns true if a keyboard has been added or removed
pub fn discover_available_keyboards(app_handle: &AppHandle) -> Result<bool, String> {
    let mut has_been_updated = false;

    let mut available_keyboards = xavkeyboardandmousegrabber::list_available_keyboards();
    /*
      Windows names a keyboard after its driver class, so every device came back as
      "HID Keyboard Device". A keyboard is keyed on its name and its path, so the resolved
      name has to replace the reported one on both the listing and the opened device,
      otherwise the two would no longer agree on a key.
    */
    for keyboard_properties in &mut available_keyboards {
        keyboard_properties.device_name = friendly_device_name(
            &keyboard_properties.device_path, &keyboard_properties.device_name, DeviceKind::Keyboard);
    }

    let state = app_handle.state::<Arc<Mutex<BackendGlobalState>>>();
    let mut state = match state.lock() {
        Ok(state) => state,
        Err(err) => return log_lock_error(format!("Lock error when attempting to get available keyboards: {err}"), app_handle), // Early return
    };
    let remembered_keyboards = state.hard_disk_storage.remembered_keyboards.clone();
    let keyboards = &mut state.keyboards_info_map;

    // Add new available keyboards
    for keyboard_properties in &available_keyboards {
        let mut is_physical_keyboard = false;
        if let Some(keyboard) = keyboards.get(&keyboard_properties.get_key()) {
            is_physical_keyboard = keyboard.keyboard.is_real_device();
        }
        if is_physical_keyboard {
            // Nothing to do
        } else {
            let keyboard_result = xavkeyboardandmousegrabber::get_keyboard(keyboard_properties.device_path.to_string(), false);
            match keyboard_result {
                Ok(mut keyboard) => {
                    keyboard.device_name = friendly_device_name(
                        &keyboard.device_path, &keyboard.device_name, DeviceKind::Keyboard);
                    // Remembered keyboards are matched on their path alone, because a
                    // resolved name can change between versions of this app, a path cannot.
                    let default_active: bool = remembered_keyboards.iter().any(|remembered_keyboard|
                        remembered_keyboard.id == keyboard.device_path);
                    keyboards.insert(keyboard.get_key(), KeyboardInfo {
                        keyboard,
                        active: default_active,
                        virtual_keyboard: None,
                    });
                    has_been_updated = true;
                },
                Err(error) => {
                    log::error!("Error when reading keyboard {} for path {} : {}", keyboard_properties.device_name, keyboard_properties.device_path, error);
                    backend_add_log(
                        format!("Error when reading keyboard {} for path {} : {}", keyboard_properties.device_name, keyboard_properties.device_path, error),
                        LogLevel::Error,
                        app_handle
                    );
                },
            }
        }
    }

    // Free keyboards no longer available
    for (key, keyboard_info) in &mut *keyboards {
        if !available_keyboards.iter().any(|available_keyboard| available_keyboard.get_key() == *key) {
            has_been_updated = true;
            if keyboard_info.keyboard.is_grabbed() {
                let _ = keyboard_info.keyboard.ungrab();
            }
        }
    }
    // Remove physical keyboard no longer available
    keyboards.retain(|key, keyboard|
        available_keyboards.iter().any(|available_keyboard| available_keyboard.get_key() == *key
            || keyboard.virtual_keyboard.is_some()));


    // Before returning, update the keyboards on the frontend
    let response: Vec<ActiveKeyboardBackendResponse> = keyboards.values().map(|keyboard_info| ActiveKeyboardBackendResponse {
        name: keyboard_info.keyboard.device_name.to_string(),
        id: keyboard_info.keyboard.device_path.to_string(),
        active: keyboard_info.active,
    }).collect();
    drop(state); // For optimisation
    to_frontend_update_keyboard_devices(response, app_handle);

    Ok(has_been_updated)
}

// Returns true if a keyboard has been activated or inactivated
pub fn update_active_keyboards(updated_keyboards: Vec<ActiveKeyboardBackendResponse>, app_handle: &AppHandle) -> Result<bool, String> {
    let mut has_been_updated = false;

    let state = app_handle.state::<Arc<Mutex<BackendGlobalState>>>();
    let mut state = match state.lock() {
        Ok(state) => state,
        Err(err) => return log_lock_error(format!("Lock error when attempting to update active keyboards: {err}"), app_handle), // Early return
    };
    let keyboards = &mut state.keyboards_info_map;

    let mut keyboards_to_remember = vec!();
    let mut keyboards_to_forget = vec!();

    // Update keyboards that have a difference in the active status
    for keyboard_info in (*keyboards).values_mut() {
        let updated_keyboard = match updated_keyboards.iter().find(|keyboard|
            keyboard.id == keyboard_info.keyboard.device_path && keyboard.name == keyboard_info.keyboard.device_name
        ) {
            Some(updated_keyboard) => updated_keyboard,
            None => continue, // Nothing to update, this would be an invalid keyboard
        };
        if keyboard_info.active != updated_keyboard.active {
            has_been_updated = true;
            keyboard_info.active = updated_keyboard.active;
            if updated_keyboard.active {
                keyboards_to_remember.push(RememberedDevice {
                    id: updated_keyboard.id.clone(),
                    name: updated_keyboard.name.clone(),
                });
            } else {
                keyboards_to_forget.push(RememberedDevice {
                    id: updated_keyboard.id.clone(),
                    name: updated_keyboard.name.clone(),
                });
            }
        }
    }

    // Update configuration to remember user choices. If a choice is not explicitly removed by the user, it is remembered.
    // Matched on the id, which is the device path, so a device the user chose keeps being
    // remembered when its resolved name changes.
    state.hard_disk_storage.remembered_keyboards.retain(
        |keyboard| !keyboards_to_forget.iter().any(|keybord_to_forget| keybord_to_forget.id == keyboard.id));
    for keyboard_to_remember in keyboards_to_remember {
        state.hard_disk_storage.remembered_keyboards.retain(
            |remembered_keyboard| remembered_keyboard.id != keyboard_to_remember.id);
        state.hard_disk_storage.remembered_keyboards.push(keyboard_to_remember);
    }
    let config = state.hard_disk_storage.clone();
    let _ = save_config(app_handle, config);
    drop(state);

    Ok(has_been_updated)
}


pub fn fetch_keyboard_events(app_handle: &AppHandle) {
    let state = app_handle.state::<Arc<Mutex<BackendGlobalState>>>();
    let mut state = match state.lock() {
        Ok(state) => state,
        Err(err) => { log_lock_error_void(format!("Lock error when fetching keyboard events: {err}"), app_handle); return; }, // Early return
    };
    let peer = state.network_info.discovered_apps.iter().find(|(_key, app)|
        app.info.authorized_by_self && app.info.authorized_by_peer
        && state.network_info.self_info.focused_id == app.info.id
    );
    let focused_peer_id = match peer {
        Some(app) => app.1.info.id.clone(),
        None => "".to_string(),
    };

    let keyboards = &mut state.keyboards_info_map;

    let mut requests_to_send: Vec<NetworkApplicationRequest> = vec!();
    for keyboard_info in (*keyboards).values_mut() {
        if !keyboard_info.keyboard.is_real_device() {
            continue; // No events to fetch for a virtual only keyboard
        }
        if !keyboard_info.active {
            if keyboard_info.keyboard.is_grabbed() {
                let _ = keyboard_info.keyboard.ungrab();
            }
            continue; // Don't send events for inactive keyboards
        }

        // Grab or ungrab keyboard depending on focus
        if !focused_peer_id.is_empty() {
            if !keyboard_info.keyboard.is_grabbed() {
                let _ = keyboard_info.keyboard.grab();
            }
        } else if keyboard_info.keyboard.is_grabbed() {
            let _ = keyboard_info.keyboard.ungrab();
        }

        match keyboard_info.keyboard.get_recent_events() {
            Ok(events) => {
                // Nothing to do if focus is already on the current app
                if focused_peer_id.is_empty() {
                    continue;
                }

                // Optimization: no need to send a request when there is no events
                if events.is_empty() {
                    continue;
                }

                // For simplicity, supported keys are generated automatically on the app creating a virtual device
                let mut keyboard_properties = keyboard_info.keyboard.get_keyboard_properties_without_keys();
                let virtual_keyboard_name = format!("{} Virtual Keyboard", keyboard_properties.device_name);
                keyboard_properties.device_name = virtual_keyboard_name;
                let content = KeyboardEventRequestContent {
                    events,
                    keyboard_properties,
                };
                let request_content = match postcard::to_allocvec(&content) {
                    Ok(request_content) => request_content,
                    Err(_) => { // Should never happen
                        return; // Early return
                    },
                };
                requests_to_send.push(NetworkApplicationRequest {
                    to_id: focused_peer_id.clone(),
                    action: NetworkAction::KeyboardEvent,
                    content: request_content,
                });
            }
            Err(err) => {
                let error = format!("Error when reading events for {}: {}", keyboard_info.keyboard.device_name, err);
                log::debug!("{}", error.to_string());
                // Not logged, because this error is very common.
                // backend_add_log(error.to_string(), &app_handle);
            }
        }
    }

    for request in requests_to_send {
        submit_network_request(request, &mut state.network_info);
    }
}

pub fn execute_keyboard_events(app_handle: &AppHandle) {
    let state = app_handle.state::<Arc<Mutex<BackendGlobalState>>>();
    let mut state = match state.lock() {
        Ok(state) => state,
        Err(err) => { log_lock_error_void(format!("Lock error when fetching keyboard events: {err}"), app_handle); return; }, // Early return
    };
    let keyboards_events: std::collections::VecDeque<KeyboardEventRequestContent> = state.received_keyboards_events_queue.clone();
    state.received_keyboards_events_queue.clear();
    drop(state); // Necessary to prevent a deadlock

    for keyboard_event in keyboards_events {
        let _ = send_events_to_keyboard(keyboard_event.events, keyboard_event.keyboard_properties, app_handle);
    }
}

pub fn send_events_to_keyboard(events: Vec<key_events::KeyEvent>, mut keyboard_properties: KeyboardProperties, app_handle: &AppHandle) -> Result<(), String> {
    let state = app_handle.state::<Arc<Mutex<BackendGlobalState>>>();
    let mut state = match state.lock() {
        Ok(state) => state,
        Err(err) => return log_lock_error(format!("Lock error when attempting to send events to keyboards: {err}"), app_handle), // Early return
    };
    let keyboards = &mut state.keyboards_info_map;

    // If device is a virtual keyboard from another machine, it needs to be created.
    if keyboards.get(&keyboard_properties.get_key()).is_none() {
        if keyboard_properties.supported_keys.is_empty() {
            keyboard_properties.supported_keys = xavkeyboardandmousegrabber::key_events::get_default_supported_keys();
        }

        let virtual_keyboard_info = KeyboardInfo {
            keyboard: xavkeyboardandmousegrabber::Keyboard::new_uninitialized(&keyboard_properties),
            active: true,
            virtual_keyboard: None, // Will be created in the code below
        };
        keyboards.insert(keyboard_properties.get_key(), virtual_keyboard_info);
    }

    if let Some(keyboard_info) = keyboards.get_mut(&keyboard_properties.get_key()) {
        if keyboard_info.virtual_keyboard.is_none() {
            let virtual_keyboard_result = xavkeyboardandmousegrabber::VirtualKeyboardBuilder::new()
                .delay_ms(0)
                .name(keyboard_properties.device_name.to_string())
                .set_supported_keys(&keyboard_properties.supported_keys)
                .build();

            match virtual_keyboard_result {
                Ok(virtual_keyboard) => {
                    keyboard_info.virtual_keyboard = Some(virtual_keyboard);
                },
                Err(err) => {
                    let error = format!("Error when creating {}: {}", keyboard_properties.device_name, err);
                    log::error!("{}", error.to_string());
                    backend_add_log(error.to_string(), LogLevel::Error, app_handle);
                    return Err(error); // Early return
                },
            }
        }

        if let Some(virtual_keyboard) = &mut keyboard_info.virtual_keyboard {
            for event in events {
                match virtual_keyboard.send_key_event(event.code, event.state.clone()) {
                    Ok(_) => (),
                    Err(err) => {
                        let error = format!("Error when sending key {} {:?} to {}: {}", event.code, event.state, keyboard_properties.device_name, err);
                        log::error!("{}", error.to_string());
                        backend_add_log(error.to_string(), LogLevel::Error, app_handle);
                        return Err(error); // Early return
                    },
                }
            }
        }
    }

    Ok(())
}