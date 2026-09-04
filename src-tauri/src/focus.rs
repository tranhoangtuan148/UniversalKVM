use tauri::AppHandle;

use crate::common::{to_frontend_update_self_app};
use universalkvm_input::MouseMovement;
use crate::mouses::{get_all_apps_monitors};
use crate::states::{AppResponse, BorderPortal, FocusEventRequestContent, NetworkAction, NetworkApplicationRequest, NetworkInfo, SetOfMonitorsEventRequestContent};


/// The peer the cursor is currently on, or an empty string when it is on this machine.
///
/// `discovered_apps` is keyed on the very id being looked for, so the answer is one hash
/// lookup. The input loops ask for it on every pass, which is why it does not scan.
pub fn focused_peer_id(network_info: &NetworkInfo) -> String {
    let focused_id = &network_info.self_info.focused_id;
    match network_info.discovered_apps.get(focused_id) {
        Some(app) if app.info.authorized_by_self && app.info.authorized_by_peer => focused_id.clone(),
        _ => String::new(),
    }
}

pub fn send_focus_with_position(focused_id: String, position: MouseMovement, network_info: &mut NetworkInfo, app_handle: &AppHandle) {
    network_info.self_info.focused_id = focused_id.clone();
    broadcast_focus(focused_id.clone(), Some(position), None, network_info);

    // On Windows, keyboard events cannot be captured globally with raw input api. The window requires the focus to receive keyboard events.
    // As a workaround, set the focus to the app whenever the focus is redirected.
    #[cfg(target_os = "windows")]
    let _ = universalkvm_input::globals::set_focus_to_hwnd_window();

    let response: AppResponse = AppResponse::from(&network_info.self_info);
    to_frontend_update_self_app(response, app_handle);
}

/// If border is None, focus is sent without moving the other application's cursor.
pub fn send_focus_with_border(focused_id: String, border_portal: Option<BorderPortal>, network_info: &mut NetworkInfo, app_handle: &AppHandle) {
    network_info.self_info.focused_id = focused_id.clone();
    broadcast_focus(focused_id.clone(), None, border_portal, network_info);

    // On Windows, keyboard events cannot be captured globally with raw input api. The window requires the focus to receive keyboard events.
    // As a workaround, set the focus to the app whenever the focus is redirected.
    #[cfg(target_os = "windows")]
    let _ = universalkvm_input::globals::set_focus_to_hwnd_window();

    let response: AppResponse = AppResponse::from(&network_info.self_info);
    to_frontend_update_self_app(response, app_handle);
}

/// Broadcast focus to trusted peers
pub fn broadcast_focus(focused_id: String, cursor: Option<MouseMovement>, border_portal: Option<BorderPortal>, network_info: &mut NetworkInfo) {
    let trusted_peers = get_trusted_peers(network_info);
    for id in trusted_peers {
        let mut content = FocusEventRequestContent {
            focused_id: focused_id.clone(),
            position: None,
            border_portal: None,
        };
        if id == focused_id {
            content.position = cursor.clone();
            content.border_portal = border_portal.clone();
        }

        let request_content = match postcard::to_allocvec(&content) {
            Ok(request_content) => request_content,
            Err(_) => { // Should never happen
                continue; // Early return
            },
        };
        if let Some(app) = network_info.discovered_apps.get_mut(&id) {
            app.requests_queue.push_back(NetworkApplicationRequest {
                to_id: app.info.id.clone(),
                action: NetworkAction::FocusEvent,
                content: request_content,
            });
        }
    }
    network_info.signal_requests_queue.notify_one();
}

/// Broadcast updated set of monitors to trusted peers
pub fn broadcast_set_of_monitors(network_info: &mut NetworkInfo) {
    let app_monitors = get_all_apps_monitors(network_info);
    let trusted_peers = get_trusted_peers(network_info);
    for id in trusted_peers {
        let content = SetOfMonitorsEventRequestContent {
            apps: app_monitors.clone(),
            borders: network_info.borders.clone(),
        };
        let request_content = match postcard::to_allocvec(&content) {
            Ok(request_content) => request_content,
            Err(_) => { // Should never happen
                continue; // Early return
            },
        };
        if let Some(app) = network_info.discovered_apps.get_mut(&id) {
            app.requests_queue.push_back(NetworkApplicationRequest {
                to_id: app.info.id.clone(),
                action: NetworkAction::SetOfMonitorsEvent,
                content: request_content,
            });
        }
    }
    network_info.signal_requests_queue.notify_one();
}

pub fn get_trusted_peers(network_info: &NetworkInfo) -> Vec<String> {
    let mut trusted_apps: Vec<String> = vec!();
    for app in network_info.discovered_apps.values() {
        if is_trusted(&app.info.id, network_info) {
            trusted_apps.push(app.info.id.clone());
        }
    }
    trusted_apps
}

/// A trusted peer is a peer that is indirectly connected, or directly connected, to the current app
pub fn is_trusted(id: &String, network_info: &NetworkInfo) -> bool {
    get_redirection_id(id, network_info).is_some()
}

/// Returns the next id for a redirection.
///   - If to_id is the current id, None is returned
///   - If to_id is a known authenticated peer, to_id is returned
///   - If a path is known, an authorized peer id is returned
///   - Otherwise, None is returned
pub fn get_redirection_id(to_id: &String, network_info: &NetworkInfo) -> Option<String> {
    let self_id = &network_info.self_info.id;
    if *self_id == *to_id {
        return None;
    }

    if let Some(app) = network_info.discovered_apps.get(to_id) {
        let is_fully_authorized = app.info.authorized_by_self && app.info.authorized_by_peer;
        if is_fully_authorized {
            return Some(to_id.clone());
        } else {
            // If there exists a known path, return the first authorized neighbor in a possible path.
            let mut partial_paths = vec!(vec!(to_id.clone()));
            let mut visited_peers = vec!();
            while let Some(path) = partial_paths.pop() {
                let visited_peer = path.last().unwrap().clone();
                if visited_peers.contains(&visited_peer) {
                    continue;
                }
                visited_peers.push(visited_peer.clone());

                if let Some(app) = network_info.discovered_apps.get(&visited_peer) {
                    for authorized_neighbor in &app.info.connected_ids {
                        if *authorized_neighbor == *self_id {
                            // If we find the current id from an authorized neighbor, a path has been found
                            if app.info.authorized_by_self && app.info.authorized_by_peer {
                                return Some(path.last().unwrap().clone());
                            } else {
                                continue;
                            }
                        }
                        if visited_peers.contains(authorized_neighbor) {
                            continue;
                        }
                        let mut new_path = path.clone();
                        new_path.push(authorized_neighbor.clone());
                        partial_paths.push(new_path);
                    }
                }
            }
        }
    }

    None
}
