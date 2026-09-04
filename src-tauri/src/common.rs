use std::sync::{Arc, Mutex};

use tauri::{AppHandle, Emitter, Manager};

use crate::networking::{submit_broadcast_network_request};
use crate::states::{ActiveKeyboardBackendResponse, ActiveMouseBackendResponse, AppResponse, BackendGlobalState, BordersResponse, HardDiskStorage, LogLevel, LogResponse, NetworkAction, NetworkApplicationBroadcastRequest};

/*
  Ask Windows for a 1 ms timer.

  The input loop sleeps 1 ms between passes, but Windows hands out a ~15.6 ms timer by
  default: measured on Windows 11, that sleep really took 15.6 ms, so mouse movement left
  this machine in 15.6 ms lumps at 64 Hz and the cursor stuttered on the machine being
  driven. With the request, the same sleep takes about 2.6 ms.

  The cost is a higher timer interrupt rate, and therefore a little more power, for as long
  as the app runs. That is the trade a program whose whole job is forwarding input should
  make. Windows drops the request when the process exits.
*/
pub fn ask_for_a_high_resolution_timer() {
    #[cfg(target_os = "windows")] {
        const TIMERR_NOERROR: u32 = 0;
        let result = unsafe { windows::Win32::Media::timeBeginPeriod(1) };
        if result != TIMERR_NOERROR {
            log::warn!("Windows refused a 1 ms timer ({result}). Forwarded input will be coarser.");
        }
    }
}

/*
  Milliseconds since the process started, from a clock that only ever moves forward.

  The wall clock is the wrong one to measure an interval with: NTP can step it, and
  resuming from sleep can step it a long way. Code that gates itself on "has enough time
  passed" reads a backward step as no time passing at all, and stops doing its work until
  the wall clock catches up. Instant cannot step, so it cannot cause that.
*/
static PROCESS_START: std::sync::LazyLock<std::time::Instant> =
    std::sync::LazyLock::new(std::time::Instant::now);

pub fn monotonic_ms() -> u64 {
    PROCESS_START.elapsed().as_millis() as u64
}

pub fn backend_add_log(log: String, level: LogLevel, app_handle: &AppHandle) {
    log::debug!("backend: backend_add_log");
    match app_handle.emit(
        "backend-add-log",
        LogResponse {
            message: log.to_string(),
            level: level as u8,
        },
    ) {
        Ok(_) => (),
        Err(err) => log::error!("backend: failed to send log {} to frontend: {}", log, err),
    }
}

/// This struct memorizes the current time when created, and prints a log before being dropped, if there was a timeout
pub struct TimeMonitor<'a> {
    label: &'static str,
    start_ms: u64,
    timeout_ms: u64,
    app_handle: &'a AppHandle,
}
impl<'a> TimeMonitor<'a> {
    /// If it takes more time to drop the TimeMonitor than the DEFAULT_TIMEOUT_MS, then a log will be generated.
    ///
    /// The label is borrowed, not owned: every call site passes a literal, and the input
    /// loops build one of these on every pass, so an owned label was an allocation per
    /// millisecond spent on a message that is almost never printed.
    pub fn new(label: &'static str, app_handle: &'a AppHandle) -> TimeMonitor<'a> {
        TimeMonitor { label, start_ms: monotonic_ms(), timeout_ms: DEFAULT_TIMEOUT_MS, app_handle }
    }

    /// If it takes more time to drop the TimeMonitor than the timeout_ms, then a log will be generated.
    pub fn build(timeout_ms: u64, label: &'static str, app_handle: &'a AppHandle) -> TimeMonitor<'a> {
        TimeMonitor { label, start_ms: monotonic_ms(), timeout_ms, app_handle }
    }
}
impl<'a> Drop for TimeMonitor<'a> {
    fn drop(&mut self) {
        let time = monotonic_ms().saturating_sub(self.start_ms);
        if time > self.timeout_ms {
            let log = format!("{}: took {} ms (>{} ms)", self.label, time, self.timeout_ms);
            log::debug!("{}", log);
            backend_add_log(log.clone(), LogLevel::Debug, self.app_handle);
        }
    }
}
pub const DEFAULT_TIMEOUT_MS: u64 = 25;

/// Always return an Error for convenience
pub fn log_lock_error<T>(error: String, app_handle: &AppHandle) -> Result<T, String> {
    log::error!("{}", error.to_string());
    backend_add_log(error.to_string(), LogLevel::Error, app_handle);
    Err(error)
}

/// Used to log a lock error for a function returning void
pub fn log_lock_error_void(error: String, app_handle: &AppHandle) {
    log::error!("{}", error.to_string());
    backend_add_log(error.to_string(), LogLevel::Error, app_handle);
}

pub fn to_frontend_update_keyboard_devices(content: Vec<ActiveKeyboardBackendResponse>, app_handle: &AppHandle) {
    match app_handle.emit(
        "to-frontend-update-keyboard-devices",
        content
    ) {
        Ok(_) => (),
        Err(err) => {
            let error = format!("Failed to emit to_frontend_update_keyboard_devices: {}", err);
            log::error!("{}", error.to_string());
            backend_add_log(error.to_string(), LogLevel::Debug, app_handle);
        }
    }
}

pub fn to_frontend_update_mouse_devices(content: Vec<ActiveMouseBackendResponse>, app_handle: &AppHandle) {
    match app_handle.emit(
        "to-frontend-update-mouse-devices",
        content
    ) {
        Ok(_) => (),
        Err(err) => {
            let error = format!("Failed to emit to_frontend_update_mouse_devices: {}", err);
            log::error!("{}", error.to_string());
            backend_add_log(error.to_string(), LogLevel::Debug, app_handle);
        }
    }
}

pub fn to_frontend_update_borders(content: BordersResponse, app_handle: &AppHandle) {
    match app_handle.emit(
        "to-frontend-update-borders",
        content
    ) {
        Ok(_) => (),
        Err(err) => {
            let error = format!("Failed to emit to_frontend_update_borders: {}", err);
            log::error!("{}", error.to_string());
            backend_add_log(error.to_string(), LogLevel::Debug, app_handle);
        }
    }
}

pub fn to_frontend_update_self_app(content: AppResponse, app_handle: &AppHandle) {
    match app_handle.emit(
        "to-frontend-update-self-app",
        content
    ) {
        Ok(_) => (),
        Err(err) => {
            let error = format!("Failed to emit to_frontend_update_self_app: {}", err);
            log::error!("{}", error.to_string());
            backend_add_log(error.to_string(), LogLevel::Debug, app_handle);
        }
    }
}

pub fn to_frontend_update_discovered_apps(content: Vec<AppResponse>, app_handle: &AppHandle) {
    match app_handle.emit(
        "to-frontend-update-discovered-apps",
        content
    ) {
        Ok(_) => (),
        Err(err) => {
            let error = format!("Failed to emit to_frontend_update_discovered_apps: {}", err);
            log::error!("{}", error.to_string());
            backend_add_log(error.to_string(), LogLevel::Debug, app_handle);
        }
    }
}

/// Blocking function to refresh discovered apps from current global state
pub async fn to_frontend_auto_update_discovered_apps(app_handle: &AppHandle) {
    // Ensures there is no deadlock
    {
        let state = app_handle.state::<Arc<Mutex<BackendGlobalState>>>();
        let mut state = match state.lock() {
            Ok(state) => state,
            Err(err) => return log_lock_error_void(format!("Lock error when attempting to update discovered apps: {err}"), app_handle), // Early return
        };
        let network_info = &mut state.network_info;

        let response: Vec<AppResponse> = network_info.discovered_apps.values().map(|app|
            AppResponse::from(&app.info)).collect();
        drop(state); // For optimisation
        to_frontend_update_discovered_apps(response, app_handle);
    }
    submit_broadcast_network_request(NetworkApplicationBroadcastRequest {
        action: NetworkAction::RequestBroadcast,
        content: vec!(), // No content required when requesting other apps to broadcast
    }).await;
}

pub fn to_frontend_update_config(config: HardDiskStorage, app_handle: &AppHandle) {
    match app_handle.emit(
        "backend-update-configuration",
        config,
    ) {
        Ok(_) => (),
        Err(err) => log::error!("backend: failed to update config with frontend: {}", err),
    }
}