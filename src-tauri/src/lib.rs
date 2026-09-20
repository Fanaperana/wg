mod audio;
mod openai;

use audio::CaptureState;
use serde_json::Value;
use tauri::State;

/// Send a prompt (with optional prior conversation) to ChatGPT.
#[tauri::command]
async fn ask_chatgpt(
    api_key: String,
    model: String,
    messages: Value,
) -> Result<String, String> {
    if api_key.trim().is_empty() {
        return Err("Missing OpenAI API key".into());
    }
    openai::chat(&api_key, &model, messages).await
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
        .invoke_handler(tauri::generate_handler![
            ask_chatgpt,
            start_capture,
            stop_capture_and_transcribe,
            is_recording
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
