mod audio;
mod copilot;
mod openai;

use audio::CaptureState;
use copilot::{CopilotState, DeviceInfo};
use serde_json::Value;
use tauri::State;

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

/// Begin capturing system (loopback) audio.
#[tauri::command]
fn start_capture(state: State<'_, CaptureState>) -> Result<(), String> {
    state.start()
}

/// Stop capturing audio, transcribe it, and return the recognized text.
#[tauri::command]
async fn stop_capture_and_transcribe(
    api_key: String,
    model: String,
    state: State<'_, CaptureState>,
) -> Result<String, String> {
    if api_key.trim().is_empty() {
        return Err("Missing OpenAI API key".into());
    }
    let wav = state.stop()?;
    if wav.len() <= 44 {
        return Err("No audio was captured".into());
    }
    openai::transcribe(&api_key, &model, wav).await
}

#[tauri::command]
fn is_recording(state: State<'_, CaptureState>) -> bool {
    state.is_recording()
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(CaptureState::default())
        .manage(CopilotState::default())
        .invoke_handler(tauri::generate_handler![
            copilot_login_start,
            copilot_login_poll,
            ask_copilot,
            start_capture,
            stop_capture_and_transcribe,
            is_recording
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
