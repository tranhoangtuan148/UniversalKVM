use xavkeyboardandmousegrabber::{KeyEvent, Keyboard, KeyboardProperties, Mouse, MouseEvent, MouseProperties, VirtualKeyboard, VirtualMouse};

use std::{collections::VecDeque, sync::{Arc, Mutex}};

use crate::mouses::{get_all_apps_monitors};


#[repr(u8)]
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum LogLevel {
    Error = 1,
    Info = 8,
    Debug = 16,
}

#[derive(Clone, Debug, serde::Serialize)]
pub struct LogResponse {
    pub message: String,
    pub level: u8, // Same as enum LogLevel. Is an integer for serialization to be simple between frontend and Rust
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct BordersResponse {
    pub borders: Vec<BorderPair>,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct AppResponse {
    pub name: String,
    pub password: String,
    pub id: String,
    pub address_infos: Vec<String>,
    pub set_of_monitors: SetOfMonitors,
    pub online: bool,
    pub focused_id: String,
    pub authorized_by_self: bool,
    pub authorized_by_peer: bool,
    pub file_transfers: VecDeque<FileTransfersProgress>,
}
impl From<&AppInfo> for AppResponse {
    fn from(app_info: &AppInfo) -> Self {
        AppResponse {
            name: app_info.name.clone(),
            password: app_info.password.clone(),
            id: app_info.id.clone(),
            address_infos: app_info.address_infos.clone(),
            set_of_monitors: app_info.set_of_monitors.clone(),
            online: app_info.online,
            focused_id: app_info.focused_id.clone(),
            authorized_by_self: app_info.authorized_by_self,
            authorized_by_peer: app_info.authorized_by_peer,
            file_transfers: app_info.file_transfers.clone(),
        }
    }
}
impl From<&mut AppInfo> for AppResponse {
    fn from(app_info: &mut AppInfo) -> Self {
        AppResponse {
            name: app_info.name.clone(),
            password: app_info.password.clone(),
            id: app_info.id.clone(),
            address_infos: app_info.address_infos.clone(),
            set_of_monitors: app_info.set_of_monitors.clone(),
            online: app_info.online,
            focused_id: app_info.focused_id.clone(),
            authorized_by_self: app_info.authorized_by_self,
            authorized_by_peer: app_info.authorized_by_peer,
            file_transfers: app_info.file_transfers.clone(),
        }
    }
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct ActiveKeyboardBackendResponse {
    pub name: String,
    pub id: String,
    pub active: bool,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct ActiveMouseBackendResponse {
    pub name: String,
    pub id: String,
    pub active: bool,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct SubmitAppNetworkConfigBackendResponse {
    pub id: String,
    pub password: String,
}

#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
pub struct RememberedDevice {
    pub id: String,
    pub name: String,
}

#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
pub struct HardDiskStorageResponse {
    pub app_name: Option<String>,
    pub theme: Option<String>, // Light, Dark
    pub default_width: Option<u32>,
    pub default_height: Option<u32>,
    pub zoom: Option<u32>,
    pub enable_clipboard: Option<bool>,
    pub maximum_logs: Option<u32>,
    pub auto_connect: Option<bool>,
    pub download_path: Option<String>,
    // Omitted other_apps, is updated when clicking on Connect, Disconnect or by other actions.
    // Omitted keypair, frontend does not need to edit it
    // Omitted online, is updated when clicking on the online button
    pub password: Option<String>,
    pub remembered_keyboards: Option<Vec<RememberedDevice>>,
    pub remembered_mouses: Option<Vec<RememberedDevice>>,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct SetSelfOnlineBackendResponse {
    pub online: bool,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct SetFocusedIdBackendResponse {
    pub focused_id: String,
    pub x: i32,
    pub y: i32,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct ConnectToAppBackendResponse {
    pub id: String,
    pub password: String,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct DisconnectToAppBackendResponse {
    pub id: String,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct EditMonitorsBackendResponse {
    pub apps: Vec<AppSetOfMonitors>,
    pub borders: Vec<BorderPair>,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct DragBackendResponse {
    pub app_id: String,
    pub paths: Vec<String>,
}

pub struct KeyboardInfo {
    pub keyboard: Keyboard, // keyboard.device can be empty for a virtual keyboard only device
    pub active: bool,
    pub virtual_keyboard: Option<VirtualKeyboard>,
}

pub struct MouseInfo {
    pub mouse: Mouse, // mouse.device can be empty for a virtual mouse only device
    pub active: bool,
    pub virtual_mouse: Option<VirtualMouse>,
}

#[repr(u8)]
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum Border {
    Left = 0,
    Right = 1,
    Top = 2,
    Bottom = 3,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct MonitorBorder {
    pub start: i32,
    pub end: i32,
    pub border: u8, // same as enum Border. Is an integer for serialization to be simple between frontend and Rust
    pub monitor_index: u8,
    pub monitors_id: String,
    pub app_id: String,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct BorderPair {
    pub pair: [MonitorBorder; 2],
    pub color_r: u8,
    pub color_g: u8,
    pub color_b: u8,
}
impl BorderPair {
    pub fn is_related(&self, app_id: &str) -> bool {
        self.pair[0].app_id == app_id
        || self.pair[1].app_id == app_id
    }

    pub fn belongs(&self, app1_id: &str, app2_id: &str) -> bool {
        (self.pair[0].app_id == app1_id || self.pair[1].app_id == app1_id)
        && (self.pair[0].app_id == app2_id || self.pair[1].app_id == app2_id)
    }

    /// Returns true if apps has an association for both borders in pair
    pub fn is_subset(&self, apps: &[AppSetOfMonitors]) -> bool {
        apps.iter()
            .any(|app| app.id == self.pair[0].app_id)
        && apps.iter()
            .any(|app| app.id == self.pair[1].app_id)
    }

    /// Returns true if border pair is valid for both set of monitors
    pub fn is_applicable(&self, apps: &[AppSetOfMonitors]) -> bool {
        apps.iter()
            .any(|app| Monitor::get_monitors_id(&app.set_of_monitors.monitors) == self.pair[0].monitors_id)
        && apps.iter()
            .any(|app| Monitor::get_monitors_id(&app.set_of_monitors.monitors) == self.pair[1].monitors_id)
    }
}

// This struct contains the necessary information for an app to teleport the cursor, when receiving the focus
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct BorderPortal {
    pub position: f32, // relative position of a cursor to a border, should be between [0, 1]
    pub border: u8, // same as enum Border

    pub linked_border: u8, // border of the linked border
    pub linked_monitor_index: u8, // monitor index of the linked border
    pub linked_start: i32, // start of the linked border
    pub linked_end: i32, // end of the linked border
    pub linked_app_id: String, // app id of the linked border
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct Monitor {
    pub x: i32,
    pub y: i32,
    pub width: u16,
    pub height: u16,
    pub color_r: u8,
    pub color_g: u8,
    pub color_b: u8,
}
impl Monitor {
    /// Compares only the position and the size between two monitors
    pub fn is_physically_identical(&self, other_monitor: &Monitor) -> bool {
        self.x == other_monitor.x && self.y == other_monitor.y
        && self.width == other_monitor.width && self.height == other_monitor.height
    }

    /// Useful to guarantee consistent monitors_id and monitors indexes
    pub fn sort_monitors(monitors: &mut [Monitor]) {
        monitors.sort_by(|a, b| b.x.cmp(&a.x)
            .then(b.y.cmp(&a.y))
            .then(b.width.cmp(&a.width))
            .then(b.height.cmp(&a.height))
        );
    }

    /// Returns an id that identifies a set of monitors
    ///
    /// Assumes monitors are sorted with sort_monitors
    pub fn get_monitors_id(sorted_monitors: &[Monitor]) -> String {
        let mut id = "".to_string();
        for monitor in sorted_monitors {
            let monitor_id = format!("X{}Y{}W{}H{}", monitor.x, monitor.y, monitor.width, monitor.height);
            id.push_str(&monitor_id);
        }
        id
    }

    /// The borders drawn on one monitor of one app.
    ///
    /// Borrows rather than collecting: the cursor check asks for these on every pass, and
    /// only ever reads them, so there is nothing for a Vec of clones to earn.
    pub fn get_monitor_borders<'a>(app_id: &'a str, monitor_index: u8, borders: &'a [BorderPair])
        -> impl Iterator<Item = &'a BorderPair> + 'a {
        borders.iter().filter(move |border| {
            border.is_related(app_id)
                && border.pair.iter()
                    .find(|border| border.app_id == app_id)
                    .is_some_and(|monitor_border| monitor_border.monitor_index == monitor_index)
        })
    }
}

#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
pub struct SetOfMonitors {
    pub offset_x: i32,
    pub offset_y: i32,
    pub monitors: Vec<Monitor>,
}

#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
pub struct AppSetOfMonitors {
    pub id: String,
    pub set_of_monitors: SetOfMonitors,
}

#[derive(Clone, Debug, Default)]
pub struct AppInfo {
    pub name: String,
    pub password: String,
    pub id: String,
    pub address_infos: Vec<String>,
    pub set_of_monitors: SetOfMonitors,
    pub online: bool,
    pub focused_id: String,
    pub connected_ids: Vec<String>, // Used for redirection, by knowing the network topology
    pub authorized_by_self: bool,
    pub authorized_by_peer: bool,
    pub file_transfers: VecDeque<FileTransfersProgress>, // Used to display file transfers progress
}

#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
pub struct FileTransfersProgress {
    pub timestamp: u64, // Serves as an id and can be used to give a time estimate
    pub finished_bytes: u64,
    pub total_bytes: u64,
}

#[derive(Clone, Debug, Default)]
pub struct FileChunks {
    pub current_chunk_id: i32, // -1 indicates an error
    pub new_path: String, // Path can be renamed on conflict, e.g. /Downloads/file.txt could be renamed to /Downloads/file_copy_2.txt
    pub chunks: Vec<FileEventRequestContent>, // Serves to store chunks if they arrive unordered
}

#[derive(Clone, Debug, Default)]
pub struct ConfirmedFileChunks {
    pub current_chunk_id: i32, // -1 indicates an error
}

#[derive(Clone, Debug, Default)]
pub struct OtherAppInfo {
    pub info: AppInfo,
    pub requests_queue: VecDeque<NetworkApplicationRequest>,
    pub responses_queue: VecDeque<NetworkResponse>,
    pub received_requests_queue: VecDeque<NetworkRequest>,
    pub received_file_chunks: std::collections::HashMap<String, FileChunks>, // The key is a file path
    pub confirmed_file_chunks: std::collections::HashMap<String, ConfirmedFileChunks>, // Used to prevent sending file chunks too quickly, to avoid RAM memory spikes
    pub consecutive_failed_requests: u32,
}

#[repr(u8)]
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum NetworkAction {
    Connect = 0,
    ConnectionAccepted,
    Disconnect,
    Broadcast,
    RequestBroadcast,
    Received,

    ClipboardEvent,
    FetchClipboardEvent,
    FileEvent,
    ConfirmedFileEvent,
    FocusEvent,
    KeyboardEvent,
    MouseEvent,
    SetOfMonitorsEvent,
}
impl From<u8> for NetworkAction {
    /// Converts a byte into a NetworkAction. If the value is invalid, returns Disconnect.
    fn from(byte: u8) -> Self {
        match byte {
            val if val == NetworkAction::Connect as u8 => NetworkAction::Connect,
            val if val == NetworkAction::ConnectionAccepted as u8 => NetworkAction::ConnectionAccepted,
            val if val == NetworkAction::Disconnect as u8 => NetworkAction::Disconnect,
            val if val == NetworkAction::Broadcast as u8 => NetworkAction::Broadcast,
            val if val == NetworkAction::RequestBroadcast as u8 => NetworkAction::RequestBroadcast,
            val if val == NetworkAction::Received as u8 => NetworkAction::Received,

            val if val == NetworkAction::ClipboardEvent as u8 => NetworkAction::ClipboardEvent,
            val if val == NetworkAction::FetchClipboardEvent as u8 => NetworkAction::FetchClipboardEvent,
            val if val == NetworkAction::FileEvent as u8 => NetworkAction::FileEvent,
            val if val == NetworkAction::ConfirmedFileEvent as u8 => NetworkAction::ConfirmedFileEvent,
            val if val == NetworkAction::FocusEvent as u8 => NetworkAction::FocusEvent,
            val if val == NetworkAction::KeyboardEvent as u8 => NetworkAction::KeyboardEvent,
            val if val == NetworkAction::MouseEvent as u8 => NetworkAction::MouseEvent,
            val if val == NetworkAction::SetOfMonitorsEvent as u8 => NetworkAction::SetOfMonitorsEvent,
            _ => NetworkAction::Disconnect,
        }
    }
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct ConnectRequestContent {
    pub password: String,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct DisconnectRequestContent {
    pub is_manual_disconnect: bool,
    pub is_refused: bool,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct BroadcastRequestContent {
    pub name: String,
    pub online: bool,
    pub connected_ids: Vec<String>,
    pub set_of_monitors: SetOfMonitors,
}

#[repr(u8)]
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum ClipboardAction {
    Empty = 0,
    FileList,
    Html,
    Image,
    Text,
}
impl From<u8> for ClipboardAction {
    /// Converts a byte into a ClipboardAction. If the value is invalid, returns Empty.
    fn from(byte: u8) -> Self {
        match byte {
            val if val == ClipboardAction::FileList as u8 => ClipboardAction::FileList,
            val if val == ClipboardAction::Html as u8 => ClipboardAction::Html,
            val if val == ClipboardAction::Image as u8 => ClipboardAction::Image,
            val if val == ClipboardAction::Text as u8 => ClipboardAction::Text,
            _ => ClipboardAction::Empty,
        }
    }
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct ClipboardEventRequestContent {
    pub clipboard_action: ClipboardAction,
    pub clipboard_content: Vec<u8>,
    pub clipboard_alternative_content: Vec<u8>, // Used to contain text alternative with html content
}
impl From<Vec<u8>> for ClipboardEventRequestContent {
    /// Useful for performance, compared to serde.
    fn from(mut bytes: Vec<u8>) -> Self {
        if bytes.is_empty() {
            return ClipboardEventRequestContent {
                clipboard_action: ClipboardAction::Empty,
                clipboard_content: vec!(),
                clipboard_alternative_content: vec!(),
            }
        }
        let clipboard_action_byte = bytes.pop().unwrap();
        let clipboard_action = ClipboardAction::from(clipboard_action_byte);

        match clipboard_action {
            ClipboardAction::Html => {
                let html_len_vec: Vec<u8> = bytes.drain(bytes.len() - 4..).collect();
                let html_len_array: [u8; 4] = html_len_vec.try_into().unwrap();
                let html_len = u32::from_le_bytes(html_len_array) as usize;
                let html_text = bytes.drain(bytes.len() - html_len..).collect();

                let alternative_text = bytes;

                ClipboardEventRequestContent {
                    clipboard_action,
                    clipboard_content: html_text,
                    clipboard_alternative_content: alternative_text,
                }
            },
            _ => {
                ClipboardEventRequestContent {
                    clipboard_action,
                    clipboard_content: bytes,
                    clipboard_alternative_content: vec!(),
                }
            }
        }
    }
}
impl ClipboardEventRequestContent {
    /// Useful for performance, compared to serde.
    pub fn image_into_bytes(width: u32, height: u32, mut image_bytes: std::borrow::Cow<[u8]>) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(image_bytes.len() + 4 + 4 + 1);

        bytes.extend(image_bytes.to_mut().iter().cloned());

        bytes.extend(width.to_le_bytes());
        bytes.extend(height.to_le_bytes());
        bytes.push(ClipboardAction::Image as u8);

        bytes
    }

    /// Useful for performance, compared to serde.
    pub fn file_list_into_bytes(file_list: Vec<std::path::PathBuf>) -> Vec<u8> {
        let mut bytes = vec!();
        for file in file_list {
            let path = file.to_string_lossy().into_owned();
            let path_bytes = path.as_bytes();
            let path_bytes_len = (path_bytes.len() as u32).to_le_bytes();
            bytes.extend(path_bytes);
            bytes.extend(path_bytes_len);
        }
        bytes.push(ClipboardAction::FileList as u8);
        bytes
    }

    /// Useful for performance, compared to serde.
    pub fn html_into_bytes(html_text: String, alternative_text: String) -> Vec<u8> {
        let html_text_bytes = html_text.as_bytes();
        let html_text_len_bytes = (html_text_bytes.len() as u32).to_le_bytes();

        let alternative_text_bytes = alternative_text.as_bytes();

        let mut bytes = Vec::with_capacity(alternative_text_bytes.len() + html_text_bytes.len() + 4 + 1);

        bytes.extend(alternative_text_bytes);
        bytes.extend(html_text_bytes);
        bytes.extend(html_text_len_bytes);
        bytes.push(ClipboardAction::Html as u8);

        bytes
    }

    /// Useful for performance, compared to serde.
    pub fn text_into_bytes(text: String) -> Vec<u8> {
        let text_bytes = text.as_bytes();
        let mut bytes = Vec::with_capacity(text_bytes.len() + 1);
        bytes.extend(text_bytes);
        bytes.push(ClipboardAction::Text as u8);
        bytes
    }
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct FileEventRequestContent {
    pub chunk_id: i32, // -1 indicates an error
    pub path: String, // Relative path
    pub content: Vec<u8>,
}
impl From<Vec<u8>> for FileEventRequestContent {
    /// Useful for performance, compared to serde.
    fn from(mut bytes: Vec<u8>) -> Self {
        let chunk_id_vec: Vec<u8> = bytes.drain(bytes.len() - 4..).collect();
        let chunk_id_array: [u8; 4] = chunk_id_vec.try_into().unwrap();
        let chunk_id = i32::from_le_bytes(chunk_id_array);

        let path_len_vec: Vec<u8> = bytes.drain(bytes.len() - 2..).collect();
        let path_len_array: [u8; 2] = path_len_vec.try_into().unwrap();
        let path_len = u16::from_le_bytes(path_len_array) as usize;
        let path_slice = bytes.drain(bytes.len() - path_len..).collect();
        let path;
        unsafe {
            path = String::from_utf8_unchecked(path_slice);
        }

        FileEventRequestContent {
            chunk_id,
            path,
            content: bytes, // Remaining bytes should be exactly the file content
        }
    }
}
impl FileEventRequestContent {
    /// Useful for performance, compared to serde.
    pub fn into_bytes(chunk_id: i32, path: String, content: &[u8]) -> Vec<u8> {
        let chunk_id_bytes = chunk_id.to_le_bytes();
        let path_bytes = path.as_bytes();
        let path_len_bytes = (path_bytes.len() as u16).to_le_bytes();

        let mut bytes = Vec::with_capacity(4 + (2 + path_bytes.len()) + content.len());
        bytes.extend(content);
        bytes.extend(path_bytes);
        bytes.extend(path_len_bytes);
        bytes.extend(chunk_id_bytes);
        bytes
    }
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct ConfirmedFileEventRequestContent {
    pub chunk_id: i32, // -1 indicates an error
    pub path: String, // Relative path
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct FocusEventRequestContent {
    pub focused_id: String,
    pub position: Option<xavkeyboardandmousegrabber::MouseMovement>,
    pub border_portal: Option<BorderPortal>,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct KeyboardEventRequestContent {
    pub events: Vec<KeyEvent>,
    pub keyboard_properties: KeyboardProperties,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct MouseEventRequestContent {
    pub events: Vec<MouseEvent>,
    pub mouse_properties: MouseProperties,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct SetOfMonitorsEventRequestContent {
    pub apps: Vec<AppSetOfMonitors>,
    pub borders: Vec<BorderPair>,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct NetworkApplicationRequest {
    pub to_id: String,
    pub action: NetworkAction,
    pub content: Vec<u8>,
}
impl NetworkApplicationRequest {
    pub fn estimate_bytes(requests: &[NetworkApplicationRequest]) -> u64 {
        requests.iter()
            .map(|request| request.content.len() as u64)
            .sum()
    }
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct NetworkApplicationBroadcastRequest {
    pub action: NetworkAction,
    pub content: Vec<u8>,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct NetworkRequest {
    pub action: NetworkAction,
    pub content: Vec<u8>,
}
impl NetworkRequest {
    /// Useful for performance, compared to serde.
    pub fn all_into_bytes(network_requests: Vec<NetworkRequest>) -> Vec<u8> {
        let mut required_size = 2; // u16 to hold the number of requests
        for network_request in &network_requests {
            required_size += 1 + 4 + network_request.content.len();
        }
        let mut bytes = Vec::with_capacity(required_size);

        let requests_len_bytes = (network_requests.len() as u16).to_le_bytes();
        bytes.extend(requests_len_bytes);

        for network_request in network_requests {
            bytes.push(network_request.action as u8);
            bytes.extend((network_request.content.len() as u32).to_le_bytes());
            bytes.extend(network_request.content);
        }
        bytes
    }
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct NetworkResponse {
    // Not used, simply defined to be able to use libp2p RequestResponse protocol.
}

#[derive(Clone, Debug, Default)]
pub struct NetworkInfo {
    pub self_info: AppInfo,
    pub discovered_apps: std::collections::HashMap<String, OtherAppInfo>,
    pub borders: Vec<BorderPair>,
    pub signal_requests_queue: Arc<tokio::sync::Notify>,
    pub signal_received_requests_queue: Arc<tokio::sync::Notify>,
    pub signal_executed_requests_queue: Arc<(std::sync::Mutex<()>, std::sync::Condvar)>, // Signal is sent after a batch of requests has been executed
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct StoredOtherApp {
    pub id: String,
    pub app_name: String,
    pub auto_connect: bool,
    pub password: String,
}

#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
pub struct HardDiskStorage {
    pub app_name: String,
    pub theme: String, // Light, Dark
    pub default_width: u32,
    pub default_height: u32,
    pub zoom: u32,
    pub download_path: String,
    pub enable_clipboard: bool,
    pub maximum_logs: u32,
    pub auto_connect: bool,
    pub other_apps: Vec<StoredOtherApp>,
    pub borders: Vec<BorderPair>,
    pub monitor_offset_x: i32,
    pub monitor_offset_y: i32,
    pub keypair: Vec<u8>, // Used to have a consistent peer id across application restarts
    pub online: bool,
    pub password: String,
    pub remembered_keyboards: Vec<RememberedDevice>,
    pub remembered_mouses: Vec<RememberedDevice>,
}
impl HardDiskStorage {
    /// Load borders that are applicable
    pub fn load_borders(&self, network_info: &mut NetworkInfo) {
        // Remove borders related to current app
        network_info.borders.retain(|border| !BorderPair::is_related(border, &network_info.self_info.id));

        let apps_monitors = get_all_apps_monitors(network_info);

        for border in self.borders.iter() {
            if let Some(app1) = apps_monitors.iter().find(|app_monitors| app_monitors.id == border.pair[0].app_id)
                && let Some(app2) = apps_monitors.iter().find(|app_monitors| app_monitors.id == border.pair[1].app_id)
                && Monitor::get_monitors_id(&app1.set_of_monitors.monitors) == border.pair[0].monitors_id
                && Monitor::get_monitors_id(&app2.set_of_monitors.monitors) == border.pair[1].monitors_id
            {
                network_info.borders.push(border.clone());
            }
        }
    }

    /// Updates borders that are related to current app
    pub fn update_borders(&mut self, network_info: &NetworkInfo) {
        let apps_set_of_monitors = get_all_apps_monitors(network_info);
        // Removes borders related to current app and that match monitors_id
        self.borders.retain(|border| !border.is_related(&network_info.self_info.id)
            || !border.is_applicable(&apps_set_of_monitors));

        for border in network_info.borders.iter() {
            if border.is_related(&network_info.self_info.id) {
                self.borders.push(border.clone());
            }
        }
    }
}

#[derive(Default)]
pub struct BackendGlobalState {
    pub keyboards_info_map: std::collections::HashMap<String, KeyboardInfo>,
    pub mouses_info_map: std::collections::HashMap<String, MouseInfo>,
    pub received_keyboards_events_queue: VecDeque<KeyboardEventRequestContent>,
    pub received_mouses_events_queue: VecDeque<MouseEventRequestContent>,
    pub network_info: NetworkInfo,
    pub clipboard: Option<Arc<Mutex<arboard::Clipboard>>>, // Using Arc and Mutex because get and set can be slow for the clipboard
    pub hard_disk_storage: HardDiskStorage,
    pub frontend_ready: bool,
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clipboard_image_serialization_works() {
        let bytes = ClipboardEventRequestContent::image_into_bytes(4, 1, std::borrow::Cow::from(vec!(0, 127, 128, 255)));
        let content = ClipboardEventRequestContent::from(bytes);
        assert_eq!(content.clipboard_action, ClipboardAction::Image);
        assert_eq!(content.clipboard_content, vec!(
            0, 127, 128, 255,
            4, 0, 0, 0, // width
            1, 0, 0, 0, // height
        ));
        assert_eq!(content.clipboard_alternative_content, Vec::<u8>::new());
    }

    #[test]
    fn clipboard_image_serialization_works_with_empty_image() {
        let bytes = ClipboardEventRequestContent::image_into_bytes(0, 0, std::borrow::Cow::from(vec!()));
        let content = ClipboardEventRequestContent::from(bytes);
        assert_eq!(content.clipboard_action, ClipboardAction::Image);
        assert_eq!(content.clipboard_content, vec!(
            0, 0, 0, 0, // width
            0, 0, 0, 0, // height
        ));
        assert_eq!(content.clipboard_alternative_content, Vec::<u8>::new());
    }

    #[test]
    fn clipboard_file_list_serialization_works() {
        let bytes = ClipboardEventRequestContent::file_list_into_bytes(vec!(std::path::PathBuf::from("/user"), std::path::PathBuf::from("/etc")));
        let content = ClipboardEventRequestContent::from(bytes);
        assert_eq!(content.clipboard_action, ClipboardAction::FileList);
        assert_eq!(content.clipboard_content, vec!(
            47, 117, 115, 101, 114, // /user
            5, 0, 0, 0, // length
            47, 101, 116, 99, // /etc
            4, 0, 0, 0, // length
        ));
        assert_eq!(content.clipboard_alternative_content, Vec::<u8>::new());
    }

    #[test]
    fn clipboard_file_list_serialization_works_with_empty_list() {
        let bytes = ClipboardEventRequestContent::file_list_into_bytes(vec!());
        let content = ClipboardEventRequestContent::from(bytes);
        assert_eq!(content.clipboard_action, ClipboardAction::FileList);
        assert_eq!(content.clipboard_content, Vec::<u8>::new());
        assert_eq!(content.clipboard_alternative_content, Vec::<u8>::new());
    }

    #[test]
    fn clipboard_html_serialization_works() {
        let bytes = ClipboardEventRequestContent::html_into_bytes("<a>link</a>".to_string(), "link".to_string());
        let content = ClipboardEventRequestContent::from(bytes);
        assert_eq!(content.clipboard_action, ClipboardAction::Html);
        assert_eq!(content.clipboard_content, vec!(60, 97, 62, 108, 105, 110, 107, 60, 47, 97, 62));
        assert_eq!(content.clipboard_alternative_content, vec!(108, 105, 110, 107));
    }

    #[test]
    fn clipboard_html_serialization_works_with_empty_html() {
        let bytes = ClipboardEventRequestContent::html_into_bytes("".to_string(), "".to_string());
        let content = ClipboardEventRequestContent::from(bytes);
        assert_eq!(content.clipboard_action, ClipboardAction::Html);
        assert_eq!(content.clipboard_content, Vec::<u8>::new());
        assert_eq!(content.clipboard_alternative_content, Vec::<u8>::new());
    }

    #[test]
    fn clipboard_text_serialization_works() {
        let bytes = ClipboardEventRequestContent::text_into_bytes("let copy = true;".to_string());
        let content = ClipboardEventRequestContent::from(bytes);
        assert_eq!(content.clipboard_action, ClipboardAction::Text);
        assert_eq!(content.clipboard_content, vec!(108, 101, 116, 32, 99, 111, 112, 121, 32, 61, 32, 116, 114, 117, 101, 59));
        assert_eq!(content.clipboard_alternative_content, Vec::<u8>::new());
    }

    #[test]
    fn clipboard_text_serialization_works_with_empty_text() {
        let bytes = ClipboardEventRequestContent::text_into_bytes("".to_string());
        let content = ClipboardEventRequestContent::from(bytes);
        assert_eq!(content.clipboard_action, ClipboardAction::Text);
        assert_eq!(content.clipboard_content, Vec::<u8>::new());
        assert_eq!(content.clipboard_alternative_content, Vec::<u8>::new());
    }

    #[test]
    fn file_event_request_content_serialization_works() {
        let bytes = FileEventRequestContent::into_bytes(1000000, "/Downloads/file.txt".to_string(), &vec!(0, 127, 128));
        let content = FileEventRequestContent::from(bytes);
        assert_eq!(content.chunk_id, 1000000);
        assert_eq!(content.path, "/Downloads/file.txt".to_string(),);
        assert_eq!(content.content, vec!(0, 127, 128));
    }

    #[test]
    fn file_event_request_content_serialization_works_with_empty_content() {
        let bytes = FileEventRequestContent::into_bytes(-1, "".to_string(), &vec!());
        let content = FileEventRequestContent::from(bytes);
        assert_eq!(content.chunk_id, -1);
        assert_eq!(content.path, "".to_string(),);
        assert_eq!(content.content, Vec::<u8>::new());
    }

    #[test]
    fn all_into_bytes_works() {
        let requests = vec!(
            NetworkRequest {
                action: NetworkAction::Broadcast,
                content: vec!(12, 255),
            },
            NetworkRequest {
                action: NetworkAction::ClipboardEvent,
                content: vec!(0, 127, 128),
            },
        );
        let bytes = NetworkRequest::all_into_bytes(requests);
        assert_eq!(bytes, vec!(
            2, 0, // requests len
            NetworkAction::Broadcast as u8, 2, 0, 0, 0, 12, 255, // request 1
            NetworkAction::ClipboardEvent as u8, 3, 0, 0, 0, 0, 127, 128, // request 2
        ));
    }

    #[test]
    fn all_into_bytes_works_with_empty_request() {
        let requests = vec!(
            NetworkRequest {
                action: NetworkAction::Broadcast,
                content: vec!(),
            },
        );
        let bytes = NetworkRequest::all_into_bytes(requests);
        assert_eq!(bytes, vec!(
            1, 0, // requests len
            NetworkAction::Broadcast as u8, 0, 0, 0, 0, // request 1
        ));
    }
}
