use std::{sync::{Arc, Mutex}};
use tauri::{AppHandle, Manager};

use crate::{common::{backend_add_log, log_lock_error_void}, states::ClipboardEventRequestContent};
use crate::states::{self, ClipboardAction, LogLevel};


pub fn initialize_clipboard(app_handle: &AppHandle) {
    let state = app_handle.state::<Arc<Mutex<states::BackendGlobalState>>>();
    match state.lock() {
        Ok(mut state) => {
            match arboard::Clipboard::new() {
                Ok(clipboard) => {
                    state.clipboard = Some(Arc::new(Mutex::new(clipboard)));
                },
                Err(err) => {
                    drop(state); // For optimisation
                    let message = format!("Failed to initialize clipboard: {}", err).to_string();
                    log::error!("{}", message);
                    backend_add_log(message, LogLevel::Error, app_handle);
                }
            };
        },
        Err(err) => { log_lock_error_void(format!("Lock error when attempting to refresh frontend mouses: {err}"), app_handle); },
    };
}

pub fn get_clipboard_content(clipboard: &mut arboard::Clipboard) -> Vec<u8> {
    // In arboard, clipboard types (file_list, image, text) are not mutually exclusive

    // Prioritize image content in the clipboard
    let cliboard_get = clipboard.get();
    if let Ok(image) = cliboard_get.image() {
        let content = ClipboardEventRequestContent::image_into_bytes(image.width as u32, image.height as u32, image.bytes);
        return content;
    }

    let cliboard_get = clipboard.get();
    if let Ok(file_list) = cliboard_get.file_list() {
        let content = ClipboardEventRequestContent::file_list_into_bytes(file_list);
        return content;
    }

    let cliboard_get = clipboard.get();
    if let Ok(html) = cliboard_get.html() {
        let cliboard_get = clipboard.get();
        let mut alternative_text = "".to_string();
        if let Ok(text) = cliboard_get.text() {
            alternative_text = text;
        }
        let content = ClipboardEventRequestContent::html_into_bytes(html, alternative_text);
        return content;
    }

    let cliboard_get = clipboard.get();
    if let Ok(text) = cliboard_get.text() {
        let content = ClipboardEventRequestContent::text_into_bytes(text);
        return content;
    }

    // If there was an error or the content is empty
    vec!()
}

pub fn set_clipboard_content(clipboard: &mut arboard::Clipboard, mut clipboard_request: states::ClipboardEventRequestContent) -> Result<(), String> {
    match clipboard_request.clipboard_action {
        ClipboardAction::Empty => {
            match clipboard.clear() {
                Ok(_) => Ok(()),
                Err(err) => Err(format!("Failed to clear clipboard: {}", err).to_string()),
            }
        },
        ClipboardAction::FileList => {
            // Not implemented; what would it do ?
            // For now, do nothing.
            Ok(())
        },
        ClipboardAction::Html => {
            let html: String = match String::from_utf8(clipboard_request.clipboard_content) {
                Ok(html) => html,
                Err(err) => {
                    return Err(format!("Received invalid html clipboard content: {}", err).to_string());
                },
            };
            let alternative_text: String = match String::from_utf8(clipboard_request.clipboard_alternative_content) {
                Ok(html) => html,
                Err(_) => {
                    // Quite unlikely to have a valid html, but invalid text
                    "".to_string()
                },
            };
            match clipboard.set_html(html, Some(alternative_text)) {
                Ok(_) => Ok(()),
                Err(err) => Err(format!("Failed to set html into clipboard: {}", err).to_string()),
            }
        },
        ClipboardAction::Image => {
            let height_vec: Vec<u8> = clipboard_request.clipboard_content.drain(clipboard_request.clipboard_content.len() - 4..).collect();
            let height_array: [u8; 4] = height_vec.try_into().unwrap();
            let height = u32::from_le_bytes(height_array);

            let width_vec: Vec<u8> = clipboard_request.clipboard_content.drain(clipboard_request.clipboard_content.len() - 4..).collect();
            let width_array: [u8; 4] = width_vec.try_into().unwrap();
            let width = u32::from_le_bytes(width_array);

            let image_data = arboard::ImageData {
                width: width as usize,
                height: height as usize,
                bytes: clipboard_request.clipboard_content.into(),
            };
            match clipboard.set_image(image_data) {
                Ok(_) => Ok(()),
                Err(err) => Err(format!("Failed to set image into clipboard: {}", err).to_string()),
            }
        },
        ClipboardAction::Text => {
            let text: String = match String::from_utf8(clipboard_request.clipboard_content) {
                Ok(text) => text,
                Err(err) => {
                    return Err(format!("Received invalid text clipboard content: {}", err).to_string());
                },
            };
            match clipboard.set_text(text) {
                Ok(_) => Ok(()),
                Err(err) => Err(format!("Failed to set text into clipboard: {}", err).to_string()),
            }
        },
    }
}