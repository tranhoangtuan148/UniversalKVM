use std::io::{Read, Write};
use std::sync::{Arc, Mutex};

use tauri::{AppHandle, LogicalSize, Manager};

use gethostname::gethostname;

use crate::common::{backend_add_log, log_lock_error_void, to_frontend_update_config, to_frontend_update_discovered_apps, to_frontend_update_self_app};
use crate::{get_handle, return_back_handle};
use crate::networking::{submit_network_request, submit_network_requests};
use crate::states::{AppResponse, BackendGlobalState, ConfirmedFileChunks, ConfirmedFileEventRequestContent, DragBackendResponse, FileEventRequestContent, FileTransfersProgress, HardDiskStorage, HardDiskStorageResponse, LogLevel, NetworkAction, NetworkApplicationRequest};

extern crate dirs;

pub const CONFIG_FILENAME: &str = "universalkvm_config.txt";
pub const CONFIG_FILENAME_TEMPORARY: &str = "universalkvm_config_tmp.txt";

/// Called after an update to config, to ensures every config is applied.
/// 
/// If specific_updates is None, all updates are applied. If specific_updates is defined, only defined things are updated.
///
/// Warning: This function can block if the UI is frozen.
pub fn update_application_config(specific_updates: Option<HardDiskStorageResponse>, app_handle: &AppHandle) {
    let state: tauri::State<'_, Arc<Mutex<BackendGlobalState>>> = app_handle.state::<Arc<Mutex<BackendGlobalState>>>();
    let mut state = match state.lock() {
        Ok(state) => state,
        Err(err) => {
            let message = format!("Lock error when attempting to update app after configuration change: {err}");
            log_lock_error_void(message.clone(), app_handle);
            return;
        }, // Early return
    };
    let config = state.hard_disk_storage.clone();
    // Some data are copied directly
    state.network_info.self_info.name = config.app_name.clone();
    state.network_info.self_info.online = config.online;
    state.network_info.self_info.password = config.password.clone();
    state.network_info.self_info.set_of_monitors.offset_x = config.monitor_offset_x;
    state.network_info.self_info.set_of_monitors.offset_y = config.monitor_offset_y;
    config.load_borders(&mut state.network_info);
    let update_self: AppResponse = AppResponse::from(&state.network_info.self_info);
    drop(state);

    let mut update_theme = specific_updates.is_none();
    let mut update_windows_size = specific_updates.is_none();
    let mut update_zoom = specific_updates.is_none();

    if let Some(updates) = specific_updates {
        if updates.theme.is_some() {
            update_theme = true;
        }
        if updates.default_height.is_some() || updates.default_width.is_some() {
            update_windows_size = true;
        }
        if updates.zoom.is_some() {
            update_zoom = true;
        }
    }

    let window = app_handle.get_webview_window("main");
    if let Some(window) = window {
        if update_theme {
            let _ = window.set_theme(
                if config.theme == "Dark" { Some(tauri::Theme::Dark) } else { Some(tauri::Theme::Light) }
            );
        }
        if update_windows_size {
            let _ = window.set_size(LogicalSize {
                width: config.default_width,
                height: config.default_height
            });
        }
        if update_zoom {
            let _ = window.set_zoom(config.zoom as f64 / 100.0);
        }
    }

    to_frontend_update_config(config, app_handle);
    to_frontend_update_self_app(update_self, app_handle);
}

pub fn get_config_directory() -> Result<std::path::PathBuf, String> {
    match dirs::config_local_dir() {
        Some(pathbuf) => {
            let app_directory = pathbuf.to_path_buf()
                .join("universalkvm");
            Ok(app_directory)
        },
        None => {
            Err("Failed to find config directory".to_string())
        }
    }
}

/// Attempts to load config from CONFIG_FILENAME.
/// 
/// If specific_updates is None, all updates are applied. If specific_updates is defined, only defined things are updated.
pub fn load_config(specific_updates: Option<HardDiskStorageResponse>, app_handle: &AppHandle) -> Result<(), String> {
    let config_directory = get_config_directory();
    if let Ok(config_directory) = config_directory {
        let config_path = config_directory.join(CONFIG_FILENAME);

        let file = std::fs::File::open(&config_path);
        match file {
            Ok(file) => {
                let reader = std::io::BufReader::new(file);
                let config: HardDiskStorage = match serde_json::from_reader(reader) {
                    Ok(config) => config,
                    Err(err) => {
                        let message = format!("Parsing bug when trying to load config: {}", err);
                        log::error!("{}", message);
                        backend_add_log(message.clone(), LogLevel::Error, app_handle);
                        return Err(message); // Early return
                    },
                };

                let state = app_handle.state::<Arc<Mutex<BackendGlobalState>>>();
                let mut state = match state.lock() {
                    Ok(state) => state,
                    Err(err) => {
                        let message = format!("Lock error when attempting to load_config: {err}");
                        log_lock_error_void(message.clone(), app_handle);
                        return Err(message);
                    }, // Early return
                };
                state.hard_disk_storage = config;
                drop(state);
                update_application_config(specific_updates, app_handle);
                return Ok(());
            },
            Err(err) => {
                let message = format!("Failed to read config, or config not yet created: {}", err);
                log::warn!("{}", message);
                backend_add_log(message.clone(), LogLevel::Info, app_handle);
                return Err(message); // Early return
            },
        }
    }

    Err("Failed to load config file".to_string())
}

/// Not guaranteed to find a path.
pub fn get_download_path(app_handle: &AppHandle) -> String {
    let result = dirs::download_dir();
    match result {
        Some(base_path) => {
            match base_path.into_os_string().into_string() {
                Ok(base_path) => base_path,
                Err(err) => {
                    let message = format!("Failed to convert Downloads folder into path string: {err:?}");
                    log::error!("{}", message);
                    backend_add_log(message.to_string(), LogLevel::Error, app_handle);
                    "".to_string()
                },
            }
        },
        None => {
            let message = "Failed to find a path to the Downloads folder. To use the drag and drop functionality, a path should be defined.".to_string();
            log::error!("{}", message);
            backend_add_log(message.to_string(), LogLevel::Error, app_handle);
            "".to_string()
        }
    }
}

/// Should be used instead of [load_config] when no configuration has been applied.
pub fn load_config_or_set_default(app_handle: &AppHandle) -> Result<(), String> {
    let result= load_config(None, app_handle);
    if result.is_err() {
        let message = "No configuration file found. Using default values.".to_string();
        log::info!("{}", message);
        backend_add_log(message, LogLevel::Info, app_handle);

        let computer_name = format!("{}", gethostname().to_string_lossy());
        let default_config = HardDiskStorage {
            app_name: computer_name,
            theme: "Dark".to_string(),
            default_width: 800,
            default_height: 600,
            zoom: 100,
            download_path: get_download_path(app_handle),
            enable_clipboard: true,
            maximum_logs: 250,
            auto_connect: true,
            other_apps: vec!(),
            borders: vec!(),
            monitor_offset_x: 0,
            monitor_offset_y: 0,
            keypair: vec!(),
            online: true,
            password: "".to_string(),
            remembered_keyboards: vec!(),
            remembered_mouses: vec!(),
        };
        let state = app_handle.state::<Arc<Mutex<BackendGlobalState>>>();
        let mut state = match state.lock() {
            Ok(state) => state,
            Err(err) => {
                let message = format!("Lock error when attempting to load default config: {err}");
                log_lock_error_void(message.clone(), app_handle);
                return Err(message);
            }, // Early return
        };
        state.hard_disk_storage = default_config;
        drop(state);
        update_application_config(None, app_handle);
    }
    Ok(())
}

/// Allow to save the application configuration into a file named CONFIG_FILENAME, in the same
/// folder as the application executable.
pub fn save_config(app_handle: &AppHandle, config: HardDiskStorage) -> Result<(), String> {
    let config_directory = get_config_directory();
    if let Ok(config_directory) = config_directory {
        let _ = std::fs::create_dir_all(config_directory.clone());

        let config_temporary_path = config_directory.join(CONFIG_FILENAME_TEMPORARY);
        let config_path = config_directory.join(CONFIG_FILENAME);

        let config_text: String = match serde_json::to_string_pretty(&config) {
            Ok(data_text) => data_text,
            Err(err) => {
                let message = format!("Parsing bug when trying to save config: {}", err);
                log::error!("{}", message);
                return Err(message); // Early return
            },
        };

        // Temporary file is to reduce odds of data corruption
        let result = std::fs::write(&config_temporary_path, config_text);
        if let Err(err) = result {
            let message = format!("Failed to save config: {}", err);
            log::error!("{}", message);
            backend_add_log(message.clone(), LogLevel::Error, app_handle);
            return Err(message); // Early return
        }
        let result = std::fs::rename(config_temporary_path, config_path);
        if let Err(err) = result {
            let message = format!("Failed to replace config with temporary config: {}", err);
            log::error!("{}", message);
            backend_add_log(message.clone(), LogLevel::Error, app_handle);
            return Err(message); // Early return
        }
        return Ok(());
    }

    Err("Failed to save config file".to_string())
}


/// Determines the maximum size of a file chunk
pub const MAX_CHUNK_SIZE_MBS: u16 = 4;

/// Buffer size threshold, which determines when to flush file chunks to peer
pub const SEND_FILE_CHUNKS_THRESHOLD: u64 = 4 * 1024 * 1024;

/// Returns a list of file paths, from a list of files and folders
pub fn get_files_from_paths(mut paths: Vec<String>) -> Result<Vec<String>, String> {
    let mut files = vec!();

    // Used to prevent infinite recursion
    let mut visited_paths = std::collections::HashSet::new();

    while !paths.is_empty() {
        let path = match paths.pop() {
            Some(path) => path,
            None => break, // Should never happen
        };

        let path_info = std::path::Path::new(&path);
        if path_info.is_file() {
            files.push(path);
        } else if path_info.is_dir() {
            visited_paths.insert(path.clone());
            match path_info.read_dir() {
                Ok(entries) => {
                    for entry in entries {
                        let entry = match entry {
                            Ok(entry) => entry,
                            Err(err) => {
                                return Err(format!("Failed to read entry in directory {path}: {err}"));
                            }
                        };
                        let metadata = match entry.metadata() {
                            Ok(metadata) => metadata,
                            Err(err) => {
                                return Err(format!("Failed to read metadata in directory {path}: {err}"));
                            },
                        };
                        if metadata.is_file() || metadata.is_symlink() {
                            let entry_path = entry.path();
                            let file_path = match entry_path.to_str() {
                                Some(file_path) => file_path,
                                None => {
                                    return Err(format!("Failed to read file path in directory {path}"));
                                },
                            };
                            files.push(file_path.to_string());
                        } else if metadata.is_dir() {
                            let entry_path = entry.path();
                            let dir_path = match entry_path.to_str() {
                                Some(dir_path) => dir_path,
                                None => {
                                    return Err(format!("Failed to read directory path in directory {path}"));
                                },
                            };
                            if !visited_paths.contains(dir_path) {
                                paths.push(dir_path.to_string());
                            }
                        } else {
                            return Err(format!("An entry is neither a directory nor a file, or is inaccessible in {path}"));
                        }
                    }
                },
                Err(err) => {
                    return Err(format!("Failed to read in directory {path}: {err}"));
                }
            };
        } else {
            return Err(format!("{path} is neither a directory nor a file, or is inaccessible"));
        }
    }

    Ok(files)
}

/// Returns an estimate in bytes of the size of the files
///
/// If there are errors, the result will be underestimated
pub fn get_total_bytes_from_paths(file_paths: &Vec<String>) -> u64 {
    let mut bytes = 0;
    for file in file_paths {
        let path_info = std::path::Path::new(&file);
        match path_info.metadata() {
            Ok(metadata) => {
                bytes += metadata.len();
            },
            Err(_) => continue,
        }
    }
    bytes
}

/// This struct is used to optimise small file transfers
pub struct FileChunksBatch {
    pub chunk_requests: Vec<NetworkApplicationRequest>,
    pub confirmation_goals: std::collections::HashMap<String, i32>,
    pub files_to_be_completed: Vec<String>,
}

/// Creates a thread that sends files in chunks
pub async fn transfer_files(drag: DragBackendResponse) {
    let app_handle = get_handle();

    std::thread::spawn(move || {
        let all_paths = drag.paths.clone();
        let files = match get_files_from_paths(drag.paths) {
            Ok(files) => files,
            Err(err) => {
                log::error!("{err}");
                backend_add_log(err, LogLevel::Error, &app_handle);
                return_back_handle(app_handle);
                return; // Early return
            },
        };

        // Initiate file transfers progress
        let total_bytes = get_total_bytes_from_paths(&files);
        let file_transfers_timestamp;
        {
            let state = app_handle.state::<Arc<Mutex<BackendGlobalState>>>();
            let mut state = match state.lock() {
                Ok(state) => state,
                Err(err) => { log_lock_error_void(format!("Lock error before transferring files: {err}"), &app_handle); return; }, // Early return
            };
            let now =  std::time::SystemTime::now();
            let since_the_epoch = now.duration_since(std::time::UNIX_EPOCH).unwrap_or_default();
            file_transfers_timestamp = since_the_epoch.as_millis() as u64;
            if let Some(app) = state.network_info.discovered_apps.get_mut(&drag.app_id) {
                app.info.file_transfers.push_back(FileTransfersProgress {
                    timestamp: file_transfers_timestamp,
                    finished_bytes: 0,
                    total_bytes,
                });
            }
            // To update file transfers progress
            let response: Vec<AppResponse> = state.network_info.discovered_apps.values().map(|app|
                AppResponse::from(&app.info)).collect();
            to_frontend_update_discovered_apps(response, &app_handle);
        }

        let mut file_chunks_batch = FileChunksBatch {
            chunk_requests: vec!(),
            confirmation_goals: std::collections::HashMap::new(),
            files_to_be_completed: vec!(),
        };

        // For now, using a sequential file transfer
        for file in &files {
            let destination_path = get_shortest_relative_path(file, &all_paths);
            let success = transfer_file(
                file.clone(),
                destination_path,
                drag.app_id.clone(),
                file_transfers_timestamp,
                &mut file_chunks_batch
            );
            if !success {
                break;
            }
            if NetworkApplicationRequest::estimate_bytes(&file_chunks_batch.chunk_requests) >= SEND_FILE_CHUNKS_THRESHOLD {
                let success = send_file_chunks_and_wait_for_confirmations(&mut file_chunks_batch, &app_handle);
                if !success {
                    break;
                }
            }
        }

        // Flush any remaining chunks
        let _ = send_file_chunks_and_wait_for_confirmations(&mut file_chunks_batch, &app_handle);

        // Remove file transfers progress
        let state = app_handle.state::<Arc<Mutex<BackendGlobalState>>>();
        let mut state = match state.lock() {
            Ok(state) => state,
            Err(err) => { log_lock_error_void(format!("Lock error after transferring files: {err}"), &app_handle); return; }, // Early return
        };
        if let Some(app) = state.network_info.discovered_apps.get_mut(&drag.app_id) {
            app.info.file_transfers.retain(|file_transfers| file_transfers.timestamp != file_transfers_timestamp);
        }
        // To update file transfers progress
        let response: Vec<AppResponse> = state.network_info.discovered_apps.values().map(|app|
            AppResponse::from(&app.info)).collect();
        to_frontend_update_discovered_apps(response, &app_handle);

        drop(state);
        return_back_handle(app_handle);
    });
}

/// path must be a valid file, not a directory
///
/// Returns true if the file was transferred successfully
pub fn transfer_file(
    path: String,
    destination_path: String,
    destination_id: String,
    file_transfers_timestamp: u64,
    file_chunks_batch: &mut FileChunksBatch
) -> bool {
    let app_handle = get_handle();

    let file = std::fs::File::open(&path);
    match file {
        Ok(file) => {
            let chunk_size = match file.metadata() {
                Ok(metadata) => {
                    std::cmp::min(1024 * 1024 * MAX_CHUNK_SIZE_MBS as usize ,metadata.len() as usize)
                },
                Err(_) => 1024 * 1024 * MAX_CHUNK_SIZE_MBS as usize,
            };
            let mut reader: std::io::BufReader<std::fs::File> = std::io::BufReader::new(file);
            let mut chunk_buffer: Vec<u8> = vec![0; chunk_size];

            // Initialize confirmed file chunks
            {
                let state = app_handle.state::<Arc<Mutex<BackendGlobalState>>>();
                let mut state = match state.lock() {
                    Ok(state) => state,
                    Err(err) => { log_lock_error_void(format!("Lock error when waiting to transfer_file: {err}"), &app_handle); return false; }, // Early return
                };
                if let Some(app) = state.network_info.discovered_apps.get_mut(&destination_id) {
                    let _ = app.confirmed_file_chunks.insert(destination_path.clone(), ConfirmedFileChunks {
                        current_chunk_id: 0,
                    });
                } else {
                    let message = format!("File {path} was not sent to {destination_id}; app is not connected");
                    log::error!("{}", message);
                    backend_add_log(message.clone(), LogLevel::Error, &app_handle);
                    drop(state);
                    return_back_handle(app_handle);
                    return false; // Early return
                }
            }

            // Unbounded for loop to get a unique chunk_id
            for chunk_id in 0.. {
                let bytes_count = reader.read(&mut chunk_buffer);
                match bytes_count {
                    Ok(bytes_count) => {
                        let chunk_slice = &chunk_buffer[0..bytes_count];
                        // Not using serde for performance
                        let request_content = FileEventRequestContent::into_bytes(chunk_id, destination_path.clone(), chunk_slice);

                        {
                            let state = app_handle.state::<Arc<Mutex<BackendGlobalState>>>();
                            let mut state = match state.lock() {
                                Ok(state) => state,
                                Err(err) => { log_lock_error_void(format!("Lock error when attempting to transfer_file: {err}"), &app_handle); return false; }, // Early return
                            };
                            if let Some(app) = state.network_info.discovered_apps.get_mut(&destination_id)
                                && let Some(file_transfers) = app.info.file_transfers.iter_mut()
                                    .find(|file_transfers| file_transfers.timestamp == file_transfers_timestamp) {
                                file_transfers.finished_bytes += bytes_count as u64; // Currently, only file content is counted
                            }

                            file_chunks_batch.chunk_requests.push(NetworkApplicationRequest {
                                to_id: destination_id.clone(),
                                action: NetworkAction::FileEvent,
                                content: request_content,
                            });
                            file_chunks_batch.confirmation_goals.insert(destination_path.clone(), chunk_id);
                            if bytes_count == 0 {
                                file_chunks_batch.files_to_be_completed.push(destination_path.clone());
                            }

                            // To update file transfers progress
                            let response: Vec<AppResponse> = state.network_info.discovered_apps.values().map(|app|
                                AppResponse::from(&app.info)).collect();
                            to_frontend_update_discovered_apps(response, &app_handle);
                        }

                        if NetworkApplicationRequest::estimate_bytes(&file_chunks_batch.chunk_requests) >= SEND_FILE_CHUNKS_THRESHOLD {
                            let success = send_file_chunks_and_wait_for_confirmations(file_chunks_batch, &app_handle);
                            if !success {
                                return_back_handle(app_handle);
                                return false; // Early return
                            }
                        }

                        if bytes_count == 0 {
                            break; // Reached end of file, no need to send any more chunks
                        }
                    },
                    Err(err) => {
                        let message = format!("Failed to read chunk {chunk_id} from file {path} to send to {destination_id}: {err}");
                        log::error!("{}", message);
                        backend_add_log(message.clone(), LogLevel::Error, &app_handle);
                        return_back_handle(app_handle);
                        return false; // Early return
                    },
                }
            }
        },
        Err(err) => {
            let message = format!("Failed to read file {path} to send to {destination_id}: {err}");
            log::error!("{}", message);
            backend_add_log(message.clone(), LogLevel::Error, &app_handle);
        },
    }

    return_back_handle(app_handle);

    true
}

/// The purpose of sending multiple chunks at a time is to speed up small file transfers.
///
/// Returns false if a failure or timeout occurred.
pub fn send_file_chunks_and_wait_for_confirmations(
    file_chunks_batch: &mut FileChunksBatch,
    app_handle: &AppHandle
) -> bool {
    let app_id = match file_chunks_batch.chunk_requests.first() {
        Some(request) => request.to_id.clone(),
        None => { return false; } // Early return
    };

    {
        let state = app_handle.state::<Arc<Mutex<BackendGlobalState>>>();
        let mut state = match state.lock() {
            Ok(state) => state,
            Err(err) => { log_lock_error_void(format!("Lock error when sending file chunks: {err}"), app_handle); return false; }, // Early return
        };
        let mut requests = Vec::with_capacity(file_chunks_batch.chunk_requests.len());
        requests.append(&mut file_chunks_batch.chunk_requests);
        let submitted = submit_network_requests(requests, &mut state.network_info);
        if !submitted {
            let message = "Stopped sending file; app is not connected".to_string();
            log::error!("{}", message);
            backend_add_log(message.clone(), LogLevel::Error, app_handle);
            return false; // Early return
        }
    }

    let start =  std::time::SystemTime::now();
    let since_the_epoch = start.duration_since(std::time::UNIX_EPOCH).unwrap_or_default();
    let start_ms: u128 = since_the_epoch.as_millis();
    let no_confirmation_timeout_ms = 30 * 1000;
    loop {
        let signal_executed_requests;
        {
            let state = app_handle.state::<Arc<Mutex<BackendGlobalState>>>();
            let mut state = match state.lock() {
                Ok(state) => state,
                Err(err) => { log_lock_error_void(format!("Lock error when waiting to send file chunks: {err}"), app_handle); return false; }, // Early return
            };
            let mut ready = true;
            if let Some(app) = state.network_info.discovered_apps.get(&app_id) {
                for (path, current_chunk) in file_chunks_batch.confirmation_goals.iter() {
                    if let Some(confirmed_file_chunks) = app.confirmed_file_chunks.get(path) {
                        if *current_chunk > confirmed_file_chunks.current_chunk_id {
                            ready = false;
                        }
                        if confirmed_file_chunks.current_chunk_id == -1 {
                            let message = "Stopped sending file; other app had a write error.".to_string();
                            log::error!("{}", message);
                            backend_add_log(message.clone(), LogLevel::Info, app_handle);
                            return false; // Early return
                        }
                    }
                }
            }
            if ready {
                file_chunks_batch.confirmation_goals.clear();
                if let Some(app) = state.network_info.discovered_apps.get_mut(&app_id) {
                    for file in file_chunks_batch.files_to_be_completed.iter() {
                        let _ = app.confirmed_file_chunks.remove(file);
                    }
                    file_chunks_batch.files_to_be_completed.clear();
                }
                break;
            }
            let now =  std::time::SystemTime::now();
            let since_the_epoch = now.duration_since(std::time::UNIX_EPOCH).unwrap_or_default();
            let now_ms = since_the_epoch.as_millis();
            if now_ms - start_ms > no_confirmation_timeout_ms {
                let message = "Stopped sending file; received no confirmation within 30 seconds.".to_string();
                log::error!("{}", message);
                backend_add_log(message.clone(), LogLevel::Error, app_handle);
                return false; // Early return
            }

            signal_executed_requests = state.network_info.signal_executed_requests_queue.clone();
        }
        let lock = match signal_executed_requests.0.lock() {
            Ok(lock) => lock,
            Err(err) => { log_lock_error_void(format!("Lock error when waiting to send file chunks: {err}"), app_handle); return false; }, // Early return
        };
        let result = signal_executed_requests.1.wait_timeout(lock, std::time::Duration::from_millis(no_confirmation_timeout_ms as u64));
        match result {
            Ok(result) => {
                if result.1.timed_out() {
                    let message = "Stopped sending file; received no confirmation within 30 seconds.".to_string();
                    log::error!("{}", message);
                    backend_add_log(message.clone(), LogLevel::Error, app_handle);
                    return false; // Early return
                }
            },
            Err(err) => { log_lock_error_void(format!("Lock error after timeout when waiting to send file chunks: {err}"), app_handle); return false; }, // Early return
        }
    }

    true
}

/// Attempts to return the significant part, e.g. 'file' for
/// path='user/Downloads/file' and all_paths=['user/Downloads/file', 'user/Downloads/subFolder/otherFile']
///
/// The resulting path will be using forward slashes (/).
pub fn get_shortest_relative_path(path: &str, all_paths: &Vec<String>) -> String {
    // Ignore trailing separator
    let trimmed_path = path
        .trim_start_matches(std::path::MAIN_SEPARATOR_STR)
        .trim_end_matches(std::path::MAIN_SEPARATOR_STR);
    let parts: Vec<&str> = trimmed_path.split(std::path::MAIN_SEPARATOR_STR).collect();

    let mut last_common_index: isize = parts.len() as isize - 1;
    for other_path in all_paths {
        // Ignore trailing separator
        let trimmed_path = other_path
            .trim_start_matches(std::path::MAIN_SEPARATOR_STR)
            .trim_end_matches(std::path::MAIN_SEPARATOR_STR);
        let other_parts: Vec<&str> = trimmed_path.split(std::path::MAIN_SEPARATOR_STR).collect();

        for i in (0..(last_common_index + 1) as usize).rev() {
            if i + 2 > other_parts.len() {
                last_common_index = other_parts.len() as isize - 2;
            } else if parts[i] != other_parts[i] {
                last_common_index = i as isize - 1;
            }
        }
        if last_common_index == -1 {
            break;
        }
    }

    // The part that is common to all paths is not used
    let mut shortest_start_index= (last_common_index + 1) as usize;

    // Ensures that at least one part is used
    shortest_start_index = if shortest_start_index >= parts.len() { parts.len() - 1 } else { shortest_start_index };

    parts[shortest_start_index..].join("/")
}

/// If a file already exists on a path, then find a similar path that is available, e.g. file_copy_2.txt instead of file.txt
///
/// The relative path is assumed to have forward slash (/)
///
/// Currently, the file is always written in the download folder.
pub fn get_available_file_path(base_path: String, relative_path: &str, app_handle: &AppHandle) -> Option<String> {
    if base_path.is_empty() {
        let message = "Download path was not found or is not defined!".to_string();
        log::error!("{}", message);
        backend_add_log(message.to_string(), LogLevel::Error, app_handle);
        return None;
    }

    let relative_path = relative_path.replace("/", std::path::MAIN_SEPARATOR_STR);
    let path = format!("{}{}{}", base_path, std::path::MAIN_SEPARATOR_STR, relative_path);

    if std::fs::exists(&path).ok() == Some(false) {
        return Some(path.clone());
    }

    const MAX_INDEX: i32 = 100; // Prevent an infinite loop in case file system always yield an error
    for index in 2..MAX_INDEX {
        let mut parts: Vec<&str> = path.split(std::path::MAIN_SEPARATOR_STR).collect();
        let last_part = match parts.pop() {
            Some(last_part) => last_part.to_string(),
            None => { return None; }, // Early return, should not happen
        };

        let mut name_parts: Vec<&str> = last_part.split(".").collect();
        // For a file name like iso-2.3.deb, the part before the last part is used, yielding iso-2.3_copy_2.deb
        let part_index_to_change = if name_parts.len() > 1 {name_parts.len() - 2} else {0};
        let name_begin = format!("{}_copy_{index}", name_parts[part_index_to_change]);
        name_parts[part_index_to_change] = &*name_begin;
        let name = name_parts.join(".");

        parts.push(&*name);
        let new_path = parts.join(std::path::MAIN_SEPARATOR_STR);
        if std::fs::exists(&new_path).ok() == Some(false) {
            return Some(new_path);
        }
    }

    let message = format!("Download path {} was not found!", base_path).to_string();
    log::error!("{}", message);
    backend_add_log(message.to_string(), LogLevel::Error, app_handle);
    None
}

/// If it is the first chunk of a file, a new file is created. If a file already exists, no overwrite is done.
pub fn write_chunk_to_file(content: FileEventRequestContent, app_handle: &AppHandle) -> Result<(), String> {
    let parent_dir = std::path::Path::new(&content.path);
    if let Some(parent_dir) = parent_dir.parent() {
        // Recursively create all missing parent directories
        match std::fs::create_dir_all(parent_dir) {
            Ok(_) => (),
            Err(err) => {
                let message = format!("Failed to create parent directory for {}: {}", content.path, err);
                log::error!("{}", message);
                backend_add_log(message.clone(), LogLevel::Error, app_handle);
                return Err(message); // Early return
            },
        }
    }

    let file = std::fs::OpenOptions::new()
        .append(true)
        .create_new(content.chunk_id == 0) // The file must not exists for the first chunk, to prevent overwrite.
        .open(&content.path);
    match file {
        Ok(file) => {
            let mut writer = std::io::BufWriter::new(file);
            match writer.write_all(&content.content) {
                Ok(_) => Ok(()),
                Err(err) => {
                    let message = format!("Failed to write chunk {} to file {}: {}", content.chunk_id, content.path, err);
                    log::error!("{}", message);
                    backend_add_log(message.clone(), LogLevel::Error, app_handle);
                    Err(message)
                },
            }
        },
        Err(err) => {
            let message = format!("Failed to write to file {} (file might already exists): {}", content.path, err);
            log::error!("{}", message);
            backend_add_log(message.clone(), LogLevel::Error, app_handle);
            Err(message)
        },
    }
}

/// Creates a thread that can write available chunks to file
pub fn write_to_file(app_id: String, content: FileEventRequestContent) {
    let app_handle = get_handle();

    std::thread::spawn(move || {
        let state = app_handle.state::<Arc<Mutex<BackendGlobalState>>>();
        let mut state = match state.lock() {
            Ok(state) => state,
            Err(err) => {
                let message = format!("Lock error before writing to file: {err}");
                log_lock_error_void(message.clone(), &app_handle);
                return;
            }, // Early return
        };
        let download_path = state.hard_disk_storage.download_path.clone();
        let app_destination = match state.network_info.discovered_apps.get_mut(&app_id) {
            Some(app_destination) => app_destination,
            None => {
                drop(state);
                return_back_handle(app_handle);
                return;
            },
        };

        let path = content.path.clone();
        if let Some(file_chunks) = app_destination.received_file_chunks.get_mut(&content.path) {
            // Only reset state when the chunk 0 is received
            if content.chunk_id == 0 {
                file_chunks.current_chunk_id = 0;
                file_chunks.new_path = match get_available_file_path(download_path, &content.path, &app_handle) {
                    Some(new_path) => new_path,
                    None => {
                        file_chunks.current_chunk_id = -1;
                        file_chunks.chunks.clear();

                        // Send an error
                        let content = ConfirmedFileEventRequestContent {
                            chunk_id: file_chunks.current_chunk_id,
                            path,
                        };
                        let request_content = match postcard::to_allocvec(&content) {
                            Ok(request_content) => request_content,
                            Err(_) => { // Should never happen
                                drop(state);
                                return_back_handle(app_handle);
                                return; // Early return
                            },
                        };
                        let _ = submit_network_request(NetworkApplicationRequest {
                            to_id: app_id,
                            action: NetworkAction::ConfirmedFileEvent,
                            content: request_content.to_vec(),
                        }, &mut state.network_info);

                        drop(state);
                        return_back_handle(app_handle);
                        return; // Early return
                    },
                };
            }

            // Always use the available path
            if file_chunks.current_chunk_id >= 0 {
                file_chunks.chunks.push(content); // Only keep chunks if there is no error
            }
        }

        // Avoid blocking the global state
        drop(state);

        // Attempt to write any possible unordered chunk
        loop {
            let state = app_handle.state::<Arc<Mutex<BackendGlobalState>>>();
            let mut state = match state.lock() {
                Ok(state) => state,
                Err(err) => {
                    let message = format!("Lock error when writing to file: {err}");
                    log_lock_error_void(message.clone(), &app_handle);
                    return;
                }, // Early return
            };
            let app_destination = match state.network_info.discovered_apps.get_mut(&app_id) {
                Some(app_destination) => app_destination,
                None => {
                    drop(state);
                    return_back_handle(app_handle);
                    return;
                },
            };

            let mut chunk;
            if let Some(file_chunks) = app_destination.received_file_chunks.get_mut(&path) {
                if let Some(index) = file_chunks.chunks.iter()
                    .position(|chunk| chunk.chunk_id == file_chunks.current_chunk_id
                ) {
                    chunk = file_chunks.chunks.remove(index);
                    if chunk.content.is_empty() {
                        app_destination.received_file_chunks.remove(&path);

                        drop(state);
                        return_back_handle(app_handle);
                        return; // Early return; no need to write anything else when all chunks have been seen
                    }

                    // Always use the available path
                    chunk.path = file_chunks.new_path.clone();
                } else {
                    drop(state);
                    return_back_handle(app_handle);
                    return; // No more chunk is available to write
                }
            } else {
                drop(state);
                return_back_handle(app_handle);
                return; // Early return
            }

            // Avoid blocking the global state during write
            drop(state);
            let result = write_chunk_to_file(chunk, &app_handle);

            let state = app_handle.state::<Arc<Mutex<BackendGlobalState>>>();
            let mut state = match state.lock() {
                Ok(state) => state,
                Err(err) => {
                    let message = format!("Lock error after writing to file: {err}");
                    log_lock_error_void(message.clone(), &app_handle);
                    return;
                }, // Early return
            };
            let app_destination = match state.network_info.discovered_apps.get_mut(&app_id) {
                Some(app_destination) => app_destination,
                None => {
                    drop(state);
                    return_back_handle(app_handle);
                    return;
                },
            };
            if let Some(file_chunks) = app_destination.received_file_chunks.get_mut(&path) {
                if result.is_err() {
                    file_chunks.current_chunk_id = -1;
                    file_chunks.chunks.clear(); // Remove existing chunks on error
                } else {
                    file_chunks.current_chunk_id += 1;
                }

                // Send confirmation of received chunk
                let content = ConfirmedFileEventRequestContent {
                    chunk_id: file_chunks.current_chunk_id,
                    path: path.clone(),
                };
                let request_content = match postcard::to_allocvec(&content) {
                    Ok(request_content) => request_content,
                    Err(_) => { // Should never happen
                        drop(state);
                        return_back_handle(app_handle);
                        return; // Early return
                    },
                };
                let _ = submit_network_request(NetworkApplicationRequest {
                    to_id: app_id.clone(),
                    action: NetworkAction::ConfirmedFileEvent,
                    content: request_content.to_vec(),
                }, &mut state.network_info);
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    const SEPARATOR: &str = std::path::MAIN_SEPARATOR_STR;

    #[test]
    fn get_shortest_relative_path_works_for_file() {
        let paths = vec!(format!("home{SEPARATOR}user{SEPARATOR}Downloads{SEPARATOR}doc.txt"));
        assert_eq!(get_shortest_relative_path(&paths[0], &paths), "doc.txt".to_string());
    }

    #[test]
    fn get_shortest_relative_path_works_for_files() {
        let paths = vec!(
            format!("home{SEPARATOR}user{SEPARATOR}Downloads{SEPARATOR}folder{SEPARATOR}doc.txt"),
            format!("home{SEPARATOR}user{SEPARATOR}Downloads{SEPARATOR}folder{SEPARATOR}img.jpg")
        );
        assert_eq!(get_shortest_relative_path(&paths[0], &paths), format!("doc.txt"));
        assert_eq!(get_shortest_relative_path(&paths[1], &paths), format!("img.jpg"));
    }

    #[test]
    fn get_shortest_relative_path_works_for_files_and_folders() {
        let paths = vec!(
            format!("home{SEPARATOR}user{SEPARATOR}Downloads{SEPARATOR}folder"),
            format!("home{SEPARATOR}user{SEPARATOR}Downloads{SEPARATOR}folder{SEPARATOR}doc.txt"),
            format!("home{SEPARATOR}user{SEPARATOR}Downloads{SEPARATOR}folder{SEPARATOR}img.jpg")
        );
        assert_eq!(get_shortest_relative_path(&paths[0], &paths), format!("folder"));
        assert_eq!(get_shortest_relative_path(&paths[1], &paths), format!("folder/doc.txt"));
        assert_eq!(get_shortest_relative_path(&paths[2], &paths), format!("folder/img.jpg"));
    }

    #[test]
    fn get_shortest_relative_path_works_for_files_and_folders_separator() {
        let paths = vec!(
            format!("home{SEPARATOR}user{SEPARATOR}Downloads{SEPARATOR}folder{SEPARATOR}"),
            format!("home{SEPARATOR}user{SEPARATOR}Downloads{SEPARATOR}folder{SEPARATOR}doc.txt"),
            format!("home{SEPARATOR}user{SEPARATOR}Downloads{SEPARATOR}folder{SEPARATOR}img.jpg")
        );
        assert_eq!(get_shortest_relative_path(&paths[0], &paths), format!("folder"));
        assert_eq!(get_shortest_relative_path(&paths[1], &paths), format!("folder/doc.txt"));
        assert_eq!(get_shortest_relative_path(&paths[2], &paths), format!("folder/img.jpg"));
    }

    #[test]
    fn get_shortest_relative_path_works_for_nested_stuff1() {
        let paths = vec!(
            format!("home{SEPARATOR}user{SEPARATOR}Downloads{SEPARATOR}folder{SEPARATOR}doc.txt"),
            format!("home{SEPARATOR}user{SEPARATOR}Downloads{SEPARATOR}folder{SEPARATOR}img.jpg"),
            format!("home{SEPARATOR}user{SEPARATOR}Downloads{SEPARATOR}deep{SEPARATOR}deep{SEPARATOR}doc.md"),
            format!("home{SEPARATOR}user{SEPARATOR}Downloads{SEPARATOR}root.txt"),
        );
        assert_eq!(get_shortest_relative_path(&paths[0], &paths), format!("folder/doc.txt"));
        assert_eq!(get_shortest_relative_path(&paths[1], &paths), format!("folder/img.jpg"));
        assert_eq!(get_shortest_relative_path(&paths[2], &paths), format!("deep/deep/doc.md"));
        assert_eq!(get_shortest_relative_path(&paths[3], &paths), "root.txt".to_string());
    }

    #[test]
    fn get_shortest_relative_path_works_for_nested_stuff2() {
        let paths = vec!(
            format!("home{SEPARATOR}user{SEPARATOR}Downloads{SEPARATOR}folder{SEPARATOR}"),
            format!("home{SEPARATOR}user{SEPARATOR}Downloads{SEPARATOR}folder{SEPARATOR}doc.txt"),
            format!("home{SEPARATOR}user{SEPARATOR}Downloads{SEPARATOR}folder{SEPARATOR}img.jpg"),
            format!("home{SEPARATOR}user{SEPARATOR}Downloads{SEPARATOR}deep{SEPARATOR}middle.txt"),
            format!("home{SEPARATOR}user{SEPARATOR}Downloads{SEPARATOR}deep{SEPARATOR}deep{SEPARATOR}doc.md"),
            format!("home{SEPARATOR}user{SEPARATOR}Downloads{SEPARATOR}root.txt"),
        );
        assert_eq!(get_shortest_relative_path(&paths[0], &paths), "folder".to_string());
        assert_eq!(get_shortest_relative_path(&paths[1], &paths), format!("folder/doc.txt"));
        assert_eq!(get_shortest_relative_path(&paths[2], &paths), format!("folder/img.jpg"));
        assert_eq!(get_shortest_relative_path(&paths[3], &paths), format!("deep/middle.txt"));
        assert_eq!(get_shortest_relative_path(&paths[4], &paths), format!("deep/deep/doc.md"));
        assert_eq!(get_shortest_relative_path(&paths[5], &paths), "root.txt".to_string());
    }

    #[test]
    fn get_shortest_relative_path_works_for_nested_stuff3() {
        let paths = vec!(
            format!("home{SEPARATOR}user"),
            format!("home{SEPARATOR}user{SEPARATOR}Downloads{SEPARATOR}folder{SEPARATOR}doc.txt"),
        );
        assert_eq!(get_shortest_relative_path(&paths[0], &paths), "user".to_string());
        assert_eq!(get_shortest_relative_path(&paths[1], &paths), format!("user/Downloads/folder/doc.txt"));
    }

    #[test]
    fn get_shortest_relative_path_works_for_empty_path1() {
        let paths = vec!(
            format!(""),
        );
        assert_eq!(get_shortest_relative_path(&paths[0], &paths), "".to_string());
    }

    #[test]
    fn get_shortest_relative_path_works_for_empty_path2() {
        let paths = vec!(
            format!("{SEPARATOR}home{SEPARATOR}file.txt"),
            format!(""),
        );
        assert_eq!(get_shortest_relative_path(&paths[0], &paths), format!("home/file.txt"));
        assert_eq!(get_shortest_relative_path(&paths[1], &paths), "".to_string());
    }

    #[test]
    fn get_shortest_relative_path_works_near_root1() {
        let paths = vec!(
            format!("{SEPARATOR}"),
            format!("{SEPARATOR}home{SEPARATOR}"),
            format!("{SEPARATOR}readme.md"),
        );
        assert_eq!(get_shortest_relative_path(&paths[0], &paths), format!(""));
        assert_eq!(get_shortest_relative_path(&paths[1], &paths), format!("home"));
        assert_eq!(get_shortest_relative_path(&paths[2], &paths), format!("readme.md"));
    }

    #[test]
    fn get_shortest_relative_path_works_near_root2() {
        let paths = vec!(
            format!("c:{SEPARATOR}"),
            format!("c:{SEPARATOR}home{SEPARATOR}"),
            format!("c:{SEPARATOR}readme.md"),
        );
        assert_eq!(get_shortest_relative_path(&paths[0], &paths), format!("c:"));
        assert_eq!(get_shortest_relative_path(&paths[1], &paths), format!("c:/home"));
        assert_eq!(get_shortest_relative_path(&paths[2], &paths), format!("c:/readme.md"));
    }
}
