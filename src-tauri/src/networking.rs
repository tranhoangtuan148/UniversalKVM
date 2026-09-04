use crate::{clipboard, get_handle, return_back_handle};
use crate::common::{backend_add_log, log_lock_error_void, to_frontend_update_borders, to_frontend_update_discovered_apps, to_frontend_update_self_app};
use crate::login::{add_app_if_missing, send_disconnect_from_app};
use crate::mouses::{apply_new_borders, get_position_from_border_portal, set_cursor_position };
use crate::states::{AppResponse, BackendGlobalState, BordersResponse, BroadcastRequestContent, ClipboardEventRequestContent, ConfirmedFileEventRequestContent, ConnectRequestContent, DisconnectRequestContent, FileChunks, FileEventRequestContent, FocusEventRequestContent, KeyboardEventRequestContent, LogLevel, MouseEventRequestContent, NetworkAction, NetworkApplicationBroadcastRequest, NetworkApplicationRequest, NetworkInfo, NetworkRequest, NetworkResponse, SetOfMonitorsEventRequestContent};
use crate::storage::{save_config, write_to_file};
use std::collections::HashMap;
use std::str::FromStr;
use std::{
    sync::{Arc, Mutex},
};
use tauri::{AppHandle, Manager};

use libp2p::{
    futures::AsyncReadExt,
    futures::AsyncWriteExt,
    futures::stream::StreamExt,
    futures::io::BufReader,
    request_response,
    mdns,
    noise,
    swarm::{NetworkBehaviour, SwarmEvent},
    tcp, yamux
};

#[derive(NetworkBehaviour)]
struct SimpleBehaviour {
    ping: libp2p::ping::Behaviour,
    mdns: libp2p::mdns::tokio::Behaviour,
    request_response: request_response::cbor::Behaviour<
        Vec<NetworkRequest>,
        NetworkResponse
    >,
    stream: libp2p_stream::Behaviour,
}

/// Returned handle will return true if a request has been submitted, false otherwise.
pub fn submit_network_request_locking(network_request: NetworkApplicationRequest) -> Vec<tokio::task::JoinHandle<bool>> {
    let app_handle = get_handle();

    // tokio::task is used to avoid deadlocks
    let thread_handle = tokio::task::spawn(async move {
        let result;
        {
            let state = app_handle.state::<Arc<Mutex<BackendGlobalState>>>();
            let mut state = match state.lock() {
                Ok(state) => state,
                Err(err) => { log_lock_error_void(format!("Lock error when submitting network request: {err}"), &app_handle); return false; }, // Early return
            };
            result = submit_network_request_state(network_request, &mut state);
        }
        return_back_handle(app_handle);
        result
    });

    vec!(thread_handle)
}

/// Returns true if a request has been submitted, false otherwise.
pub fn submit_network_request_state(network_request: NetworkApplicationRequest, state: &mut BackendGlobalState) -> bool {
    let network_info = &mut state.network_info;
    let app_destination = network_info.discovered_apps.get_mut(&network_request.to_id);
    if let Some(app_destination) = app_destination {
        app_destination.requests_queue.push_back(network_request);
        network_info.signal_requests_queue.notify_one();
        return true;
    }
    false
}

/// Returns true if a request has been submitted, false otherwise.
pub fn submit_network_request(network_request: NetworkApplicationRequest, network_info: &mut NetworkInfo) -> bool {
    let app_destination = network_info.discovered_apps.get_mut(&network_request.to_id);
    if let Some(app_destination) = app_destination {
        app_destination.requests_queue.push_back(network_request);
        network_info.signal_requests_queue.notify_one();
        return true;
    }
    false
}

/// Returns true if a request has been submitted, false otherwise.
///
/// This assumes that all requests are destined to the same peer.
pub fn submit_network_requests(network_requests: Vec<NetworkApplicationRequest>, network_info: &mut NetworkInfo) -> bool {
    let app_id = match network_requests.first() {
        Some(request) => &request.to_id,
        None => { return false; } // Early return
    };
    let app_destination = network_info.discovered_apps.get_mut(app_id);
    if let Some(app_destination) = app_destination {
        app_destination.requests_queue.extend(network_requests);
        network_info.signal_requests_queue.notify_one();
        return true;
    }
    false
}

// Returned handle will return true if some requests have been submitted, false otherwise.
pub async fn submit_broadcast_network_request(network_request: NetworkApplicationBroadcastRequest) -> Vec<tokio::task::JoinHandle<bool>> {
    let app_handle = get_handle();

    // tokio::task is used to avoid deadlocks
    let thread_handle = tokio::task::spawn(async move {
        let mut has_submitted = false;
        {
            let state = app_handle.state::<Arc<Mutex<BackendGlobalState>>>();
            let mut state = match state.lock() {
                Ok(state) => state,
                Err(err) => { log_lock_error_void(format!("Lock error when submitting network request: {err}"), &app_handle); return false; }, // Early return
            };
            let network_info = &mut state.network_info;

            for app_destination in network_info.discovered_apps.values_mut() {
                app_destination.requests_queue.push_back(NetworkApplicationRequest {
                    to_id: app_destination.info.id.clone(),
                    action: network_request.action.clone(),
                    content: network_request.content.clone(),
                });
                network_info.signal_requests_queue.notify_one();
                has_submitted = true;
            }
        }
        return_back_handle(app_handle);
        has_submitted
    });

    vec!(thread_handle)
}


pub struct AppDestinationInfo {
    online: bool,
    is_fully_authorized: bool,
}
/// Helper function to work with ownership of state.network_info. (to release it after this function call)
pub fn get_app_destination_info(app_destination_id: &String, network_info: &mut NetworkInfo) -> AppDestinationInfo {
    let app_destination = match network_info.discovered_apps.get_mut(app_destination_id) {
        Some(app_destination) => app_destination,
        None => {
            return AppDestinationInfo {
                online: false,
                is_fully_authorized: false,
            }
        },
    };
    let is_fully_authorized = app_destination.info.authorized_by_self && app_destination.info.authorized_by_peer;
    AppDestinationInfo {
        online: network_info.self_info.online,
        is_fully_authorized,
    }
}

/// Helper function to work with ownership of state.network_info. (to release it after this function call)
pub fn get_next_network_request(app_destination_id: &String, network_info: &mut NetworkInfo) -> Option<NetworkRequest> {
    let app_destination = match network_info.discovered_apps.get_mut(app_destination_id) {
        Some(app_destination) => app_destination,
        None => { return None; }, // Should never happen
    };
    if !app_destination.received_requests_queue.is_empty() {
        let request = app_destination.received_requests_queue.pop_front();
        return request;
    }
    None // Should never happen
}

pub fn execute_received_network_requests(app_handle: &AppHandle) {
    let state = app_handle.state::<Arc<Mutex<BackendGlobalState>>>();
    let mut state = match state.lock() {
        Ok(state) => state,
        Err(err) => { log_lock_error_void(format!("Lock error when executing received network requests: {err}"), app_handle); return; }, // Early return
    };

    let self_id = state.network_info.self_info.id.clone();

    let mut new_connected_ids: Vec<String> = state.network_info.discovered_apps.iter().filter_map(|(id, app)| {
        if app.info.authorized_by_self && app.info.authorized_by_peer {
            return Some(id.clone());
        }
        None
    }).collect();

    // Cloning keys to easily move ownership of network_info. Only a peer with something
    // waiting is worth a key: this runs on every batch received, and the peer that sent the
    // batch is usually the only one with anything to execute.
    let apps_keys: Vec<String> = state.network_info.discovered_apps.iter()
        .filter(|(_id, app)| !app.received_requests_queue.is_empty())
        .map(|(id, _app)| id.clone())
        .collect();
    for app_destination_id in apps_keys {
        while let Some(request) = get_next_network_request(&app_destination_id, &mut state.network_info) {
            let AppDestinationInfo {
                online,
                is_fully_authorized,
            } = get_app_destination_info(&app_destination_id, &mut state.network_info);
            let from_id = &app_destination_id;

            match request.action {
                NetworkAction::Connect => {
                    if !online { continue; } // Only check connection request if online

                    let content: ConnectRequestContent = match postcard::from_bytes(&request.content) {
                        Ok(content) => content,
                        Err(err) => { log::debug!("Invalid received connect request: {err}"); continue }, // Early return
                    };
                    let authorized = content.password == state.network_info.self_info.password;
                    let to_id;
                    {
                        let app_destination = match state.network_info.discovered_apps.get_mut(&app_destination_id) {
                            Some(app_destination) => app_destination,
                            None => { continue }, // Should never happen
                        };
                        app_destination.info.authorized_by_peer = true;

                        let message = format!("{} connection from {}", if authorized {"Accepted"} else {"Refused"}, from_id);
                        log::info!("{}", message);
                        backend_add_log(message.to_string(), LogLevel::Info, app_handle);
                        app_destination.info.authorized_by_self = authorized;

                        to_id = app_destination.info.id.clone();
                    }

                    if authorized {
                        let app_destination = match state.network_info.discovered_apps.get_mut(&app_destination_id) {
                            Some(app_destination) => app_destination,
                            None => { continue }, // Should never happen
                        };
                        app_destination.requests_queue.push_back(NetworkApplicationRequest {
                            to_id,
                            action: NetworkAction::ConnectionAccepted,
                            content: vec!(),
                        });
                    } else {
                        send_disconnect_from_app(
                            &to_id,
                            Some(DisconnectRequestContent {
                                is_manual_disconnect: false,
                                is_refused: true,
                            }),
                            &mut state.network_info,
                            app_handle
                        );
                    }
                    state.network_info.signal_requests_queue.notify_one();
                },
                NetworkAction::ConnectionAccepted => {
                    let app_destination = match state.network_info.discovered_apps.get_mut(&app_destination_id) {
                        Some(app_destination) => app_destination,
                        None => { continue }, // Should never happen
                    };
                    app_destination.info.authorized_by_peer = true;
                },
                NetworkAction::Disconnect => {
                    let mut should_remove_auto_connect = false;
                    if let Ok(content) = postcard::from_bytes::<DisconnectRequestContent>(&request.content) {
                        should_remove_auto_connect = content.is_manual_disconnect || content.is_refused;
                    }
                    if should_remove_auto_connect {
                        // Because of state lock, update config manually
                        if let Some(stored_app) = state.hard_disk_storage.other_apps.iter_mut().find(|app| app.id == *from_id) {
                            stored_app.auto_connect = false;
                        }
                        let config = state.hard_disk_storage.clone();
                        let _ = save_config(app_handle, config);
                    }
                    send_disconnect_from_app(
                        from_id,
                        None, // No need to send back a disconnect request
                        &mut state.network_info,
                        app_handle,
                    );

                    let message = format!("Disconnected from {}", from_id);
                    log::info!("{}", message);
                    backend_add_log(message.to_string(),LogLevel::Info, app_handle);
                },
                NetworkAction::Broadcast => {
                    let app_destination = match state.network_info.discovered_apps.get_mut(&app_destination_id) {
                        Some(app_destination) => app_destination,
                        None => { continue }, // Should never happen
                    };
                    let content: BroadcastRequestContent = match postcard::from_bytes(&request.content) {
                        Ok(content) => content,
                        Err(err) => { log::debug!("Invalid received broadcast request: {err}"); continue }, // Early return
                    };

                    let sanitized_name = content.name.chars().take(50).collect();
                    app_destination.info.name = sanitized_name;
                    app_destination.info.online = content.online;
                    app_destination.info.connected_ids = content.connected_ids;
                    app_destination.info.set_of_monitors = content.set_of_monitors;

                    // Health check: if other app is disconnected, do a soft disconnect
                    if is_fully_authorized && !app_destination.info.connected_ids.contains(&self_id) {
                        send_disconnect_from_app(
                            from_id,
                            None, // No need to send a disconnect request
                            &mut state.network_info,
                            app_handle,
                        );
                    }

                    let config = state.hard_disk_storage.clone();
                    config.load_borders(&mut state.network_info);
                    let borders_response = BordersResponse { borders: state.network_info.borders.clone() };
                    to_frontend_update_borders(borders_response, app_handle);
                    log::debug!("Received broadcast info from peer {}", from_id);
                },
                NetworkAction::RequestBroadcast => {
                    let content = BroadcastRequestContent {
                        name: state.network_info.self_info.name.clone(),
                        online: state.network_info.self_info.online,
                        connected_ids: new_connected_ids.clone(),
                        set_of_monitors: state.network_info.self_info.set_of_monitors.clone(),
                    };
                    let app_destination = match state.network_info.discovered_apps.get_mut(&app_destination_id) {
                        Some(app_destination) => app_destination,
                        None => { continue }, // Should never happen
                    };
                    let request_content = match postcard::to_allocvec(&content) {
                        Ok(request_content) => request_content,
                        Err(_) => { // Should never happen
                            continue; // Early return
                        },
                    };
                    app_destination.requests_queue.push_back(NetworkApplicationRequest {
                        to_id: app_destination.info.id.clone(),
                        action: NetworkAction::Broadcast,
                        content: request_content,
                    });
                    state.network_info.signal_requests_queue.notify_one();
                    log::debug!("Received broadcast request from peer {}", from_id);
                },
                NetworkAction::Received => {
                    // Not used
                },
                NetworkAction::ClipboardEvent => {
                    if !is_fully_authorized { continue; }

                    if !state.hard_disk_storage.enable_clipboard {
                        let message = "Clipboard is not enabled: content refused.".to_string();
                        log::info!("{}", message);
                        backend_add_log(message,LogLevel::Info, app_handle);
                        continue;
                    }

                    // Using a thread because clipboard set can be slow
                    let clipboard_app_handle = get_handle();
                    std::thread::spawn(move || {
                        let content = ClipboardEventRequestContent::from(request.content);

                        let state = clipboard_app_handle.state::<Arc<Mutex<BackendGlobalState>>>();
                        let mut state = match state.lock() {
                            Ok(state) => state,
                            Err(err) => { log_lock_error_void(format!("Lock error before setting the clipboard: {err}"), &clipboard_app_handle); return; }, // Early return
                        };

                        if let Some(clipboard) = &mut state.clipboard {
                            let clipboard = clipboard.clone();
                            drop(state); // Main thread should not be blocked

                            match clipboard.lock() {
                                Ok(mut clipboard) => {
                                    let result = clipboard::set_clipboard_content(&mut clipboard, content);
                                    if let Err(err) = result {
                                        log::error!("Failed to set clipboard content: {}", err);
                                        backend_add_log(err,LogLevel::Error, &clipboard_app_handle);
                                    }
                                    return_back_handle(clipboard_app_handle);
                                },
                                Err(err) =>  { log_lock_error_void(format!("Lock error when setting the clipboard: {err}"), &clipboard_app_handle); }, // Early return
                            }
                        } else {
                            log::error!("No clipboard is available to copy into.");
                            drop(state);
                            return_back_handle(clipboard_app_handle);
                        }
                    });
                },
                NetworkAction::FetchClipboardEvent => {
                    if !is_fully_authorized { continue; }

                    if !state.hard_disk_storage.enable_clipboard {
                        let message = "Clipboard is not enabled: refused to send content.".to_string();
                        log::info!("{}", message);
                        backend_add_log(message,LogLevel::Info, app_handle);
                        continue;
                    }

                    let to_id = app_destination_id.clone();

                    // Using a thread because clipboard get can be slow
                    let clipboard_app_handle = get_handle();
                    std::thread::spawn(move || {
                        let mut clipboard_request_content = None;

                        {
                            let state = clipboard_app_handle.state::<Arc<Mutex<BackendGlobalState>>>();
                            let mut state = match state.lock() {
                                Ok(state) => state,
                                Err(err) => { log_lock_error_void(format!("Lock error before getting the clipboard: {err}"), &clipboard_app_handle); return; }, // Early return
                            };
                            if let Some(clipboard) = &mut state.clipboard {
                                let clipboard = clipboard.clone();
                                drop(state); // Main thread should not be blocked

                                match clipboard.lock() {
                                    Ok(mut clipboard) => {
                                        let content = clipboard::get_clipboard_content(&mut clipboard);
                                        clipboard_request_content = Some(NetworkApplicationRequest {
                                            to_id,
                                            action: NetworkAction::ClipboardEvent,
                                            content,
                                        });
                                    },
                                    Err(err) =>  { log_lock_error_void(format!("Lock error when getting the clipboard: {err}"), &clipboard_app_handle); return; }, // Early return
                                }
                            } else {
                                log::error!("No clipboard is available to read from.");
                            }
                        }

                        let state = clipboard_app_handle.state::<Arc<Mutex<BackendGlobalState>>>();
                        let mut state = match state.lock() {
                            Ok(state) => state,
                            Err(err) => { log_lock_error_void(format!("Lock error before sending the clipboard: {err}"), &clipboard_app_handle); return; }, // Early return
                        };
                        if let Some(clipboard_request_content) = clipboard_request_content
                            && let Some(app_destination) = state.network_info.discovered_apps.get_mut(&clipboard_request_content.to_id) {
                            app_destination.requests_queue.push_back(clipboard_request_content);
                            state.network_info.signal_requests_queue.notify_one();
                        }

                        drop(state);
                        return_back_handle(clipboard_app_handle);
                    });
                },
                NetworkAction::ConfirmedFileEvent => {
                    if !is_fully_authorized { continue; }

                    let content: ConfirmedFileEventRequestContent = match postcard::from_bytes(&request.content) {
                        Ok(content) => content,
                        Err(err) => { log::debug!("Invalid received file event request: {err}"); continue }, // Early return
                    };
                    let app_destination = match state.network_info.discovered_apps.get_mut(&app_destination_id) {
                        Some(app_destination) => app_destination,
                        None => { continue }, // Should never happen
                    };
                    if let Some(confirmed_file_chunks) = app_destination.confirmed_file_chunks.get_mut(&content.path) {
                        if content.chunk_id == -1 || confirmed_file_chunks.current_chunk_id == -1 {
                            confirmed_file_chunks.current_chunk_id = -1;
                            continue; // If there is an error, let the thread 'transfer_file' manage the error
                        }
                        if content.chunk_id > confirmed_file_chunks.current_chunk_id {
                            confirmed_file_chunks.current_chunk_id = content.chunk_id;
                        }
                    }
                },
                NetworkAction::FileEvent => {
                    if !is_fully_authorized { continue; }

                    // Not using serde for performance
                    let content: FileEventRequestContent = FileEventRequestContent::from(request.content);
                    let app_destination = match state.network_info.discovered_apps.get_mut(&app_destination_id) {
                        Some(app_destination) => app_destination,
                        None => { continue }, // Should never happen
                    };
                    if !app_destination.received_file_chunks.contains_key(&content.path) {
                        app_destination.received_file_chunks.insert(content.path.clone(), FileChunks {
                            current_chunk_id: 0,
                            new_path: "".to_string(),
                            chunks: vec!(),
                        });
                    }

                    write_to_file(app_destination_id.clone(), content);
                },
                NetworkAction::FocusEvent => {
                    if !is_fully_authorized { continue; }

                    let content: FocusEventRequestContent = match postcard::from_bytes(&request.content) {
                        Ok(content) => content,
                        Err(err) => { log::debug!("Invalid received focus event: {err}"); continue }, // Early return
                    };

                    let focused_peer_is_fully_authorized = match state.network_info.discovered_apps.get(&content.focused_id) {
                        Some(app) => {
                            app.info.authorized_by_self && app.info.authorized_by_peer
                        }
                        None => content.focused_id == self_id,
                    };

                    let new_focused_id = if focused_peer_is_fully_authorized {content.focused_id.clone()} else {self_id.clone()};

                    if state.network_info.self_info.focused_id != new_focused_id {
                        let prev_focused_id = state.network_info.self_info.focused_id.clone();
                        state.network_info.self_info.focused_id = new_focused_id;

                        let response = AppResponse::from(&state.network_info.self_info);
                        to_frontend_update_self_app(response, app_handle);

                        // To manage clipboard automatic update, clipboard is fetched when focus is transferred to current app.
                        if state.network_info.self_info.focused_id == self_id && state.hard_disk_storage.enable_clipboard {
                            submit_network_request_state(NetworkApplicationRequest {
                                to_id: prev_focused_id,
                                action: NetworkAction::FetchClipboardEvent,
                                content: vec!(),
                            }, &mut state);
                        }
                    }

                    if let Some(border_portal) = content.border_portal
                        && let Some(position) = get_position_from_border_portal(
                        border_portal, &state.network_info.self_info.set_of_monitors.monitors) {
                        // Using a thread because global state is locked and set_cursor_position could block.
                        let cursor_app_handle = get_handle();
                        std::thread::spawn(move || {
                            let _ = set_cursor_position(position.x, position.y, &cursor_app_handle);
                            return_back_handle(cursor_app_handle);
                        });
                    } else if let Some(position) = content.position {
                        // Using a thread because global state is locked and set_cursor_position could block.
                        let cursor_app_handle = get_handle();
                        std::thread::spawn(move || {
                            let _ = set_cursor_position(position.x, position.y, &cursor_app_handle);
                            return_back_handle(cursor_app_handle);
                        });
                    }
                },
                NetworkAction::KeyboardEvent => {
                    if !is_fully_authorized { continue; }

                    let content: KeyboardEventRequestContent = match postcard::from_bytes(&request.content) {
                        Ok(content) => content,
                        Err(err) => { log::debug!("Invalid received keyboard events: {err}"); continue }, // Early return
                    };

                    state.received_keyboards_events_queue.push_back(content);
                },
                NetworkAction::MouseEvent => {
                    if !is_fully_authorized { continue; }

                    let content: MouseEventRequestContent = match postcard::from_bytes(&request.content) {
                        Ok(content) => content,
                        Err(err) => { log::debug!("Invalid received mouse events: {err}"); continue }, // Early return
                    };

                    state.received_mouses_events_queue.push_back(content);
                },
                NetworkAction::SetOfMonitorsEvent => {
                    if !is_fully_authorized { continue; }

                    let content: SetOfMonitorsEventRequestContent = match postcard::from_bytes(&request.content) {
                        Ok(content) => content,
                        Err(err) => { log::debug!("Invalid received set of monitors events: {err}"); continue }, // Early return
                    };

                    apply_new_borders(&mut state, content.borders, content.apps, app_handle);

                    let self_response: AppResponse = AppResponse::from(&mut state.network_info.self_info);
                    let response: Vec<AppResponse> = state.network_info.discovered_apps.values().map(|app|
                        AppResponse::from(&app.info)).collect();
                    to_frontend_update_self_app(self_response, app_handle);
                    to_frontend_update_discovered_apps(response, app_handle);
                },
            }
        }
    }

    // Recalculated, because it can be different after receiving requests
    new_connected_ids = state.network_info.discovered_apps.iter().filter_map(|(id, app)| {
        if app.info.authorized_by_self && app.info.authorized_by_peer {
            return Some(id.clone());
        }
        None
    }).collect();
    state.network_info.self_info.connected_ids = new_connected_ids;
    state.network_info.signal_executed_requests_queue.1.notify_all();
}

/// Returns true if there is one or more received requests. This function can be cancelled and should not write to the global state.
async fn check_received_network_requests(app_handle: &AppHandle) -> Result<bool, ()> {
    let signal_received_request;
    {
        let state = app_handle.state::<Arc<Mutex<BackendGlobalState>>>();
        let state = match state.lock() {
            Ok(state) => state,
            Err(err) => { log_lock_error_void(format!("Lock error when checking received network requests: {err}"), app_handle); return Ok(false); }, // Early return
        };
        signal_received_request = state.network_info.signal_received_requests_queue.clone();
    }

    signal_received_request.notified().await;
    Ok(true)
}

/// Returns true if there is one or more requests to be sent. This function can be cancelled and should not write to the global state.
async fn check_network_requests(app_handle: &AppHandle) -> Result<bool, ()> {
    let signal_requests;
    {
        let state = app_handle.state::<Arc<Mutex<BackendGlobalState>>>();
        let state = match state.lock() {
            Ok(state) => state,
            Err(err) => { log_lock_error_void(format!("Lock error when checking network requests: {err}"), app_handle); return Ok(false); }, // Early return
        };
        signal_requests = state.network_info.signal_requests_queue.clone();
    }

    signal_requests.notified().await;
    Ok(true)
}

pub struct PeerStreamQueues {
    pub input_senders: HashMap<libp2p::PeerId, tokio::sync::mpsc::UnboundedSender<Vec<NetworkRequest>>>,
    pub bulk_senders: HashMap<libp2p::PeerId, tokio::sync::mpsc::UnboundedSender<Vec<NetworkRequest>>>,
}

fn dispatch_stream_requests(
    destination_id: libp2p::PeerId,
    requests: Vec<NetworkRequest>,
    senders_map: &mut HashMap<libp2p::PeerId, tokio::sync::mpsc::UnboundedSender<Vec<NetworkRequest>>>,
    swarm: &libp2p::Swarm<SimpleBehaviour>,
    stream_type: &'static str,
) {
    if requests.is_empty() {
        return;
    }

    if let Some(tx) = senders_map.get(&destination_id) {
        if !tx.is_closed() {
            let _ = tx.send(requests);
            return;
        }
    }

    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<Vec<NetworkRequest>>();
    let _ = tx.send(requests);
    senders_map.insert(destination_id, tx);

    let mut control = swarm.behaviour().stream.new_control();
    tokio::spawn(async move {
        let mut stream = match control.open_stream(destination_id, STREAM_PROTOCOL).await {
            Ok(stream) => stream,
            Err(err) => {
                log::error!("Failed to open {stream_type} stream to {destination_id}: {err}");
                return;
            }
        };

        while let Some(batch) = rx.recv().await {
            let mut all = batch;
            while let Ok(more) = rx.try_recv() {
                all.extend(more);
            }
            if let Err(err) = send_network_requests_to_stream(all, &mut stream).await {
                log::error!("Failed to send on {stream_type} stream to {destination_id}: {err}");
                let _ = stream.close().await;
                break;
            }
        }
    });
}

async fn send_network_requests(
    peer_stream_queues: &mut PeerStreamQueues,
    swarm: &mut libp2p::Swarm<SimpleBehaviour>,
    app_handle: &AppHandle,
) {
    let mut requests_to_send: std::collections::HashMap<libp2p::PeerId, Vec<NetworkRequest>> = HashMap::new();

    {
        let state = app_handle.state::<Arc<Mutex<BackendGlobalState>>>();
        let mut state = match state.lock() {
            Ok(state) => state,
            Err(err) => return log_lock_error_void(format!("Lock error when sending network requests: {err}"), app_handle), // Early return
        };
        let network_info = &mut state.network_info;

        for app_destination in &mut network_info.discovered_apps.values_mut() {
            if app_destination.requests_queue.is_empty() {
                continue;
            }

            let destination_id = libp2p::PeerId::from_str(&app_destination.info.id);
            let destination_id = match destination_id {
                Ok(destination_id) => destination_id,
                Err(err) => {
                    let error = format!("Failed to generate peer ID to send network request: {err}");
                    log::error!("{}", error.to_string());
                    backend_add_log(error.to_string(), LogLevel::Error, app_handle);
                    continue; // Early return
                },
            };

            let mut network_requests = Vec::with_capacity(app_destination.requests_queue.len());
            while let Some(request) = app_destination.requests_queue.pop_front() {
                log::debug!("Sending message {:?} to {}", request.action, request.to_id);
                network_requests.push(NetworkRequest {
                    action: request.action,
                    content: request.content,
                });
            }

            requests_to_send.insert(destination_id, network_requests);
        }
    }

    // Send requests after state lock is dropped
    for (destination_id, network_requests) in requests_to_send {
        let (normal_requests, stream_requests): (Vec<_>, Vec<_>) = network_requests.into_iter()
            .partition(|network_request|
                network_request.action == NetworkAction::Connect
                || network_request.action == NetworkAction::ConnectionAccepted
                || network_request.action == NetworkAction::Disconnect
                || network_request.action == NetworkAction::Broadcast
                || network_request.action == NetworkAction::RequestBroadcast
            );

        if !normal_requests.is_empty() {
            swarm
                .behaviour_mut().request_response.send_request(&destination_id, normal_requests);
        }

        if !stream_requests.is_empty() {
            let (input_requests, bulk_requests): (Vec<_>, Vec<_>) = stream_requests.into_iter()
                .partition(|req| matches!(
                    req.action,
                    NetworkAction::MouseEvent
                    | NetworkAction::KeyboardEvent
                    | NetworkAction::FocusEvent
                    | NetworkAction::SetOfMonitorsEvent
                ));

            if !input_requests.is_empty() {
                dispatch_stream_requests(
                    destination_id,
                    input_requests,
                    &mut peer_stream_queues.input_senders,
                    swarm,
                    "input",
                );
            }

            if !bulk_requests.is_empty() {
                dispatch_stream_requests(
                    destination_id,
                    bulk_requests,
                    &mut peer_stream_queues.bulk_senders,
                    swarm,
                    "bulk",
                );
            }
        }
    }
}

fn receive_network_requests(from_id: &String, network_requests: Vec<NetworkRequest>, app_handle: &AppHandle) {
    let state = app_handle.state::<Arc<Mutex<BackendGlobalState>>>();
    let mut state = match state.lock() {
        Ok(state) => state,
        Err(err) => return log_lock_error_void(format!("Lock error when receiving network requests: {err}"), app_handle), // Early return
    };
    let network_info = &mut state.network_info;

    let app_source = network_info.discovered_apps.get_mut(from_id);
    if let Some(app_source) = app_source {
        app_source.received_requests_queue.extend(network_requests);
        network_info.signal_received_requests_queue.notify_one();
    }
}

async fn read_network_request_from_stream<R: AsyncReadExt + Unpin>(stream: &mut R) -> Result<Vec<NetworkRequest>, String> {
    let mut requests_len_bytes = [0u8; 2];
    let bytes_result = stream.read_exact(&mut requests_len_bytes).await;
    let requests_len = match bytes_result {
        Ok(_) => u16::from_le_bytes(requests_len_bytes),
        Err(err) => {
            let message = format!("Failed to read first bytes from stream: {}", err).to_string();
            return Err(message); // Early return
        },
    };

    let mut network_requests = Vec::with_capacity(requests_len as usize);
    for _ in 0..requests_len {
        let mut network_action_bytes = [0u8; 1];
        let bytes_result = stream.read_exact(&mut network_action_bytes).await;
        let network_action = match bytes_result {
            Ok(_) => NetworkAction::from(u8::from_le_bytes(network_action_bytes)),
            Err(err) => {
                let message = format!("Failed to read a request from stream: {}", err).to_string();
                return Err(message); // Early return
            },
        };

        let mut content_len_bytes = [0u8; 4];
        let bytes_result = stream.read_exact(&mut content_len_bytes).await;
        let content_len = match bytes_result {
            Ok(_) => u32::from_le_bytes(content_len_bytes),
            Err(err) => {
                let message = format!("Failed to read a request's length from stream: {}", err).to_string();
                return Err(message); // Early return
            },
        };

        let mut content: Vec<u8> = vec![0; content_len as usize];
        let result = stream.read_exact(&mut content).await;
        match result {
            Ok(_) => (),
            Err(err) => {
                let message = format!("Failed to read a request's content from stream: {}", err).to_string();
                return Err(message); // Early return
            },
        }

        network_requests.push(NetworkRequest { action: network_action, content })
    }

    Ok(network_requests)
}

async fn send_network_requests_to_stream(network_requests: Vec<NetworkRequest>, stream: &mut libp2p::Stream) -> Result<(), String> {
    let bytes = NetworkRequest::all_into_bytes(network_requests);

    if let Err(err) = stream.write_all(&bytes).await {
        return Err(format!("Failed to write to stream: {}", err));
    }
    if let Err(err) = stream.flush().await {
        return Err(format!("Failed to flush stream: {}", err));
    }
    Ok(())
}

const STREAM_PROTOCOL: libp2p::StreamProtocol = libp2p::StreamProtocol::new("/iofpstream"); // io frontend protototype stream

pub async fn networking_loop(app_handle: &AppHandle) -> Result<(), Box<dyn std::error::Error>> {
    const PING_TIMEOUT_S: u16 = 8; // Currently unused. Initially the Ping protocol was used to manage automatic disconnections, but it was unreliable.

    let mut keypair = libp2p::identity::Keypair::generate_ed25519();

    // Try to load keypair
    let state = app_handle.state::<Arc<Mutex<BackendGlobalState>>>();
    match state.lock() {
        Ok(mut state) => {
            let config= &mut state.hard_disk_storage;
            let private_key = config.keypair.get(0..32); // Only the private key bytes are required
            if let Some(private_key) = private_key {
                let private_key = private_key.to_vec();
                let result = libp2p::identity::Keypair::ed25519_from_bytes(private_key);
                match result {
                    Ok(loaded_keypair) => {
                        keypair = loaded_keypair;
                    },
                    Err(err) => {
                        let message = format!("No valid key pair found: {}. A new one will be generated", err).to_string();
                        log::info!("{}", message);
                        backend_add_log(message, LogLevel::Info, app_handle);
                    },
                }
            } else {
                let message = "No valid key pair found. A new one will be generated".to_string();
                log::info!("{}", message);
                backend_add_log(message, LogLevel::Info, app_handle);
            }
        },
        Err(err) => { log_lock_error_void(format!("Lock error when attempting to save key pair: {err}"), app_handle) },
    };

    // Save keypair
    let state = app_handle.state::<Arc<Mutex<BackendGlobalState>>>();
    match state.lock() {
        Ok(mut state) => {
            let config= &mut state.hard_disk_storage;
            if let Ok(ed25519) = keypair.clone().try_into_ed25519() {
                let keypair_bytes = ed25519.to_bytes();
                config.keypair = Vec::from(keypair_bytes);
            }
            let config = state.hard_disk_storage.clone();
            let result = save_config(app_handle, config);
            if let Err(err) = result {
                log::warn!("Failed to save key pair: {}", err);
            }
        },
        Err(err) => { log_lock_error_void(format!("Lock error when attempting to save key pair: {err}"), app_handle) },
    };

    let mut swarm = libp2p::SwarmBuilder::with_existing_identity(keypair)
        .with_tokio()
        .with_tcp(
            tcp::Config::default().nodelay(true),
            noise::Config::new,
            || {
                let mut yamux_config = yamux::Config::default();
                yamux_config.set_max_num_streams(1024); // Default is 512, which is not enough. For this application, it is not meaningful to protect against DDoS.
                yamux_config
            },
        )?
        .with_quic()
        .with_behaviour(|key| {
            let ping_config = libp2p::ping::Config::new()
                .with_interval(std::time::Duration::from_secs(5))
                .with_timeout(std::time::Duration::from_secs(PING_TIMEOUT_S as u64));
            let ping: libp2p::ping::Behaviour = libp2p::ping::Behaviour::new(ping_config);

            let mdns_config = mdns::Config {
                ttl: std::time::Duration::from_secs(30), // The value should not be too high, otherwise there is a risk of having one App desynchronized for a long time
                query_interval: std::time::Duration::from_secs(10), // Should be much lower than ttl, to prevent a valid ttl from expiring
                enable_ipv6: false,
            };
            let mdns =
                libp2p::mdns::tokio::Behaviour::new(mdns_config, key.public().to_peer_id())?;
            
            // For clipboard, useful to allow a high limit
            const MAX_REQUEST_SIZE_MBS: u64 = 1024;
            let codec = request_response::cbor::codec::Codec::default()
                .set_request_size_maximum(MAX_REQUEST_SIZE_MBS * 1024 * 1024)
                .set_response_size_maximum(MAX_REQUEST_SIZE_MBS * 1024 * 1024);
            let request_response_config = request_response::Config::default()
                .with_request_timeout(std::time::Duration::from_secs(2)); // Should not be too long, to avoid a long freeze when the other app or network is broken.
            let request_response = request_response::cbor::Behaviour::with_codec(
                codec,
                [(
                    libp2p::StreamProtocol::new("/universalkvm"),
                    request_response::ProtocolSupport::Full,
                )],
                request_response_config,
            );

            let stream = libp2p_stream::Behaviour::new();

            Ok(SimpleBehaviour { ping, mdns, request_response, stream })
        })?
        .build();

    let id = (*swarm.local_peer_id()).to_string();

    // Listen on all interfaces and whatever port the OS assigns
    swarm.listen_on("/ip4/0.0.0.0/udp/0/quic-v1".parse()?)?;
    swarm.listen_on("/ip4/0.0.0.0/tcp/0".parse()?)?;

    let mut incoming_streams = swarm
        .behaviour()
            .stream
        .new_control()
        .accept(STREAM_PROTOCOL)?;

    // Used to store senders for each peer, for efficient message sending for keyboard and mouse events
    let mut peer_stream_queues = PeerStreamQueues {
        input_senders: HashMap::new(),
        bulk_senders: HashMap::new(),
    };

    loop {
        tokio::select! {
            // Send any submitted requests
            result = check_network_requests(app_handle) => {
                if let Ok(has_requests_to_send) = result && has_requests_to_send {
                    send_network_requests(&mut peer_stream_queues, &mut swarm, app_handle).await;
                }
            },
            result = check_received_network_requests(app_handle) => {
                 if let Ok(has_received_requests) = result && has_received_requests {
                    execute_received_network_requests(app_handle);
                }
            },
            event = swarm.select_next_some() => {
                match event {
                    SwarmEvent::NewListenAddr { address, .. } => {
                        let address_string =  format!("{address:?}");
                        let message = format!("Listening on {address_string}");
                        log::debug!("{}", message);
                        backend_add_log(message.to_string(), LogLevel::Debug, app_handle); // This log might not appear on the frontend, because the frontend might not be ready

                        let state = app_handle.state::<Arc<Mutex<BackendGlobalState>>>();
                        let mut state = match state.lock() {
                            Ok(state) => state,
                            Err(err) => { log_lock_error_void(format!("Lock error when attempting to get new listening address: {err}"), app_handle); continue; }, // Early return
                        };
                        let focused_id = state.network_info.self_info.focused_id.clone();
                        // Focus should only be initialized once
                        if focused_id.is_empty() {
                            state.network_info.self_info.focused_id = id.clone();
                        }

                        let network_info = &mut state.network_info;
                        network_info.self_info.id = id.clone();
                        network_info.self_info.address_infos.push(address_string);
                        let response = AppResponse::from(&network_info.self_info);
                        drop(state); // For optimisation
                        to_frontend_update_self_app(response, app_handle);
                    },
                    SwarmEvent::ExpiredListenAddr { address, .. } => {
                        let address_string =  format!("{address:?}");
                        let message = format!("Removing listening address {address_string}");
                        log::debug!("{}", message);
                        backend_add_log(message.to_string(), LogLevel::Debug, app_handle);

                        let state = app_handle.state::<Arc<Mutex<BackendGlobalState>>>();
                        let mut state = match state.lock() {
                            Ok(state) => state,
                            Err(err) => { log_lock_error_void(format!("Lock error when attempting to remove listening address: {err}"), app_handle); continue; }, // Early return
                        };
                        let network_info = &mut state.network_info;

                        network_info.self_info.address_infos.retain(|address| *address != address_string);
                        let response = AppResponse::from(&network_info.self_info);
                        drop(state); // For optimisation
                        to_frontend_update_self_app(response, app_handle);
                    },

                    SwarmEvent::Behaviour(SimpleBehaviourEvent::Mdns(libp2p::mdns::Event::Discovered(list))) => {
                        for (peer_id, multiaddr) in list {
                            swarm.add_peer_address(peer_id, multiaddr.clone());

                            let state = app_handle.state::<Arc<Mutex<BackendGlobalState>>>();
                            let mut state = match state.lock() {
                                Ok(state) => state,
                                Err(err) => { log_lock_error_void(format!("Lock error when attempting to discover peer: {err}"), app_handle); continue; }, // Early return
                            };
                            let config = state.hard_disk_storage.clone();
                            let network_info = &mut state.network_info;

                            let app_id = peer_id.to_string();
                            let _ = add_app_if_missing(&app_id, network_info, config, app_handle);

                            // Should always work
                            if let Some(app) = network_info.discovered_apps.get_mut(&app_id) {
                                let address_string = format!("{multiaddr:?}");
                                if !app.info.address_infos.contains(&address_string) {
                                    let message = format!("Discovered peer {app_id} on {address_string}");
                                    log::info!("{}", message);
                                    backend_add_log(message.to_string(), LogLevel::Info, app_handle);

                                    app.info.address_infos.push(address_string);
                                    let response: Vec<AppResponse> = network_info.discovered_apps.values().map(|app|
                                        AppResponse::from(&app.info)).collect();
                                    to_frontend_update_discovered_apps(response, app_handle);
                                }
                            }
                        }
                    },
                    SwarmEvent::Behaviour(SimpleBehaviourEvent::Mdns(libp2p::mdns::Event::Expired(list))) => {
                        for (peer_id, multiaddr) in list {
                            peer_stream_queues.input_senders.remove(&peer_id);
                            peer_stream_queues.bulk_senders.remove(&peer_id);

                            let state = app_handle.state::<Arc<Mutex<BackendGlobalState>>>();
                            let mut state = match state.lock() {
                                Ok(state) => state,
                                Err(err) => { log_lock_error_void(format!("Lock error when attempting to remove expired peer: {err}"), app_handle); continue; }, // Early return
                            };
                            let network_info = &mut state.network_info;

                            let key = peer_id.to_string();
                            let address_string =  format!("{multiaddr:?}");

                            let message = format!("mDNS discover peer has expired: {peer_id}, {address_string}");
                            log::debug!("{}", message);
                            backend_add_log(message.to_string(), LogLevel::Debug, app_handle);

                            if let Some(app) = network_info.discovered_apps.get_mut(&key) {
                                app.info.address_infos.retain(|address| address != &address_string);
                            }

                            let response: Vec<AppResponse> = network_info.discovered_apps.values().map(|app|
                                AppResponse::from(&app.info)).collect();
                            drop(state); // For optimisation
                            to_frontend_update_discovered_apps(response, app_handle);
                        }
                    },

                    SwarmEvent::Behaviour(SimpleBehaviourEvent::RequestResponse(
                        request_response::Event::Message { message, peer, .. },
                    )) => match message {
                        request_response::Message::Request {
                            request, channel, request_id
                        } => {
                            let from_id = peer.to_string();

                            // request is a list of actual requests
                            for specific_request in &request {
                                log::debug!("Received message {:?} of request {:?}", specific_request.action, request_id);
                            }
                            // Ensures there is a discovered app to receive the updates
                            {
                                let state = app_handle.state::<Arc<Mutex<BackendGlobalState>>>();
                                let mut state = match state.lock() {
                                    Ok(state) => state,
                                    Err(err) => { log_lock_error_void(format!("Lock error when attempting to discover peer: {err}"), app_handle); continue; }, // Early return
                                };
                                let config = state.hard_disk_storage.clone();
                                let network_info = &mut state.network_info;
                                let _ = add_app_if_missing(&from_id, network_info, config, app_handle);

                                // When a request is received, it implies that the connection is working.
                                if let Some(app) = network_info.discovered_apps.get_mut(&from_id) {
                                    app.consecutive_failed_requests = 0;
                                }
                            }
                            receive_network_requests(&from_id, request, app_handle);

                            drop(channel); // No need to send a response
                        }
                        request_response::Message::Response {
                            request_id,
                            ..
                        } => {
                            // Not used
                            log::debug!("Received response of request {:?}", request_id);
                        }
                    },
                    SwarmEvent::Behaviour(SimpleBehaviourEvent::RequestResponse(
                        request_response::Event::OutboundFailure {
                            request_id, error, peer, ..
                        },
                    )) => {
                        match error {
                            // For now, assumes that these errors imply that another app closed its connection.
                            libp2p::request_response::OutboundFailure::DialFailure
                            | libp2p::request_response::OutboundFailure::ConnectionClosed
                            | libp2p::request_response::OutboundFailure::Timeout => {
                                let message = format!("Inaccessible peer {} for request {:?} because of: {}", peer, request_id, error);
                                log::debug!("{}", message);
                                // Application log is done inside the next if block, to avoid cluttering the logs.

                                let state = app_handle.state::<Arc<Mutex<BackendGlobalState>>>();
                                let mut state = match state.lock() {
                                    Ok(state) => state,
                                    Err(err) => { log_lock_error_void(format!("Lock error when attempting to remove inaccessible peer: {err}"), app_handle); continue; }, // Early return
                                };
                                let network_info = &mut state.network_info;

                                let key = peer.to_string();

                                let mut failures = 0;
                                if let Some(app) = network_info.discovered_apps.get_mut(&key) {
                                    app.consecutive_failed_requests += 1;
                                    failures = app.consecutive_failed_requests;

                                    // Inside if block to avoid logging too many times the same connection error.
                                    backend_add_log(message.to_string(), LogLevel::Debug, app_handle);
                                }
                                const FAILED_REQUESTS_THRESHOLD: u32 = 3;
                                if failures >= FAILED_REQUESTS_THRESHOLD {
                                    // Remove app if there is too many consecutive failures
                                    let removed = network_info.discovered_apps.remove_entry(&key);
                                    if removed.is_some() {
                                        let message = format!("Failed to reach peer {peer} after {FAILED_REQUESTS_THRESHOLD} attempts, removed peer.");
                                        log::info!("{}", message);
                                        backend_add_log(message.to_string(), LogLevel::Info, app_handle);
                                    }
                                }

                                // Disconnect automatically from peer
                                send_disconnect_from_app(
                                    &key,
                                    Some(DisconnectRequestContent {
                                        is_manual_disconnect: false,
                                        is_refused: false,
                                    }),
                                    network_info,
                                    app_handle,
                                );
                            },
                            // For the other errors (Io), do nothing.
                            _ => {
                                let message = format!("Timeout or IO error with peer {} for request {:?}: {}", peer, request_id, error);
                                log::debug!("{}", message);
                            },
                        };
                    },
                    SwarmEvent::ConnectionClosed { peer_id, .. } => {
                        peer_stream_queues.input_senders.remove(&peer_id);
                        peer_stream_queues.bulk_senders.remove(&peer_id);
                    },
                    SwarmEvent::Behaviour(event) => log::debug!("{event:?}"),
                    _ => {}
                };
            },

            // Accept incomming streams
            event = incoming_streams.next() => {
                // For security, maybe a stream should be opened only with an autorised peer.
                if let Some((peer, stream)) = event {
                    let app_handle_copy = get_handle();
                    tokio::spawn(async move {
                        let mut reader = BufReader::with_capacity(32 * 1024, stream);
                        loop {
                            match read_network_request_from_stream(&mut reader).await {
                                Ok(network_requests) => {
                                    let from_id = peer.to_string();

                                    // request is a list of actual requests
                                    for specific_request in &network_requests {
                                        log::debug!("Received stream message {:?}", specific_request.action);
                                    }
                                    receive_network_requests(&from_id, network_requests, &app_handle_copy);
                                }
                                Err(err) => {
                                    log::error!("Failed to read stream request: {}", err);
                                    // When there is an error, close the stream
                                    let mut stream = reader.into_inner();
                                    let _ = stream.close().await;
                                    log::error!("Closed read stream.");
                                    // After the stream is closed, end this thread
                                    break;
                                }
                            };
                        }
                        return_back_handle(app_handle_copy);
                    });
                }
            }
        }
    }
}