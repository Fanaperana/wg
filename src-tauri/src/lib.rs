mod copilot;
mod stt;

use std::path::PathBuf;

use copilot::{CopilotState, DeviceInfo};
use serde_json::Value;
use stt::SttState;
use tauri::{
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    Manager, State,
};

/// Show the widget if hidden, hide it if visible.
fn toggle_window(app: &tauri::AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        if window.is_visible().unwrap_or(false) {
            let _ = window.hide();
        } else {
            let _ = window.show();
            let _ = window.set_focus();
        }
    }
}

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
    app: tauri::AppHandle,
    token: String,
    github_token: String,
    model: String,
    messages: Value,
    state: State<'_, CopilotState>,
) -> Result<String, String> {
    if token.trim().is_empty() {
        return Err("Sign in with GitHub first".into());
    }
    copilot::chat(&app, &state, &token, &github_token, &model, messages).await
}

/// List the chat models available to the signed-in Copilot account.
#[tauri::command]
async fn copilot_models(
    token: String,
    state: State<'_, CopilotState>,
) -> Result<Vec<String>, String> {
    if token.trim().is_empty() {
        return Err("Sign in with GitHub first".into());
    }
    copilot::list_models(&state, &token).await
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
            // Enforce exclusion from screen capture/recording at runtime.
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.set_content_protected(true);
            }
            // Tray icon: the app is hidden from the taskbar, so this is the way to reach it.
            let show = MenuItem::with_id(app, "show", "Show / Hide", true, None::<&str>)?;
            let quit = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&show, &quit])?;
            TrayIconBuilder::new()
                .icon(app.default_window_icon().unwrap().clone())
                .tooltip("wg")
                .menu(&menu)
                .show_menu_on_left_click(false)
                .on_menu_event(|app, event| match event.id.as_ref() {
                    "show" => toggle_window(app),
                    "quit" => app.exit(0),
                    _ => {}
                })
                .on_tray_icon_event(|tray, event| {
                    if let TrayIconEvent::Click {
                        button: MouseButton::Left,
                        button_state: MouseButtonState::Up,
                        ..
                    } = event
                    {
                        toggle_window(tray.app_handle());
                    }
                })
                .build(app)?;
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            copilot_login_start,
            copilot_login_poll,
            ask_copilot,
            copilot_models,
            start_stt,
            stop_stt,
            is_recording
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
