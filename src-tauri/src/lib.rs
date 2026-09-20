mod copilot;
mod stt;

use std::path::PathBuf;

use copilot::{CopilotState, DeviceInfo};
use serde_json::Value;
use stt::SttState;
use tauri::{Manager, State};

/// Start the GitHub device-login flow for Copilot.
#[tauri::command]
async fn copilot_login_start() -> Result<DeviceInfo, String> {
    copilot::start_device_flow().await
}

/// Poll the device flow; returns the OAuth token once the user finishes, else null.
#[tauri::command]
async fn copilot_login_poll(device_code: String) -> Result<Option<String>, String> {
    copilot::poll_access_token(&device_code).await
}

/// Send a prompt (with optional prior conversation) to Copilot.
#[tauri::command]
async fn ask_copilot(
    token: String,
    model: String,
    messages: Value,
    state: State<'_, CopilotState>,
) -> Result<String, String> {
    if token.trim().is_empty() {
        return Err("Sign in with GitHub first".into());
    }
    copilot::chat(&state, &token, &model, messages).await
}

/// Start local speech-to-text on the system (loopback) audio.
#[tauri::command]
fn start_stt(state: State<'_, SttState>) -> Result<(), String> {
    state.start()
}

/// Stop the running speech-to-text session.
#[tauri::command]
fn stop_stt(state: State<'_, SttState>) {
    state.stop();
}

#[tauri::command]
fn is_recording(state: State<'_, SttState>) -> bool {
    state.is_recording()
}

/// Locate the bundled `models` directory, falling back to the working dir in dev.
fn resolve_models_dir(app: &tauri::App) -> PathBuf {
    if let Ok(path) = app
        .path()
        .resolve("models", tauri::path::BaseDirectory::Resource)
    {
        if path.exists() {
            return path;
        }
    }
    PathBuf::from("models")
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(CopilotState::default())
        .setup(|app| {
            let handle = app.handle().clone();
            let models_dir = resolve_models_dir(app);
            app.manage(SttState::new(handle, models_dir));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            copilot_login_start,
            copilot_login_poll,
            ask_copilot,
            start_stt,
            stop_stt,
            is_recording
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
