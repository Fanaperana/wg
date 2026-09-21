mod capture;
mod copilot;
mod stt;

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use copilot::{CopilotState, DeviceInfo};
use serde_json::Value;
use stt::SttState;
use tauri::{
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    Manager, State,
};

/// Holds the frozen full-screen frame while the region selector is open.
#[derive(Default)]
struct CaptureState(Mutex<Option<String>>);

/// Saved main-window bounds (x, y, w, h in physical px) to restore after capture.
#[derive(Default)]
struct SavedBounds(Mutex<Option<(i32, i32, u32, u32)>>);

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

/// Freeze the screen and expand the main widget into a fullscreen capture
/// overlay. Reuses the main webview (which renders and is content-protected),
/// so the overlay is excluded from screen sharing automatically. Returns the
/// frozen frame as a PNG data URL for the overlay to crop client-side.
#[tauri::command]
fn enter_capture(
    app: tauri::AppHandle,
    cap: State<'_, CaptureState>,
    saved: State<'_, SavedBounds>,
) -> Result<String, String> {
    let frame = capture::grab_primary()?;
    *cap.0.lock().unwrap() = Some(frame.clone());

    let win = app.get_webview_window("main").ok_or("no main window")?;
    let pos = win.outer_position().map_err(|e| e.to_string())?;
    let size = win.inner_size().map_err(|e| e.to_string())?;
    *saved.0.lock().unwrap() = Some((pos.x, pos.y, size.width, size.height));

    let monitor = win
        .primary_monitor()
        .map_err(|e| e.to_string())?
        .ok_or("no primary monitor")?;
    let mpos = monitor.position();
    let msize = monitor.size();
    // resizable:false blocks user resizing, not programmatic; toggle to be safe.
    let _ = win.set_resizable(true);
    win.set_position(tauri::PhysicalPosition::new(mpos.x, mpos.y))
        .map_err(|e| e.to_string())?;
    win.set_size(tauri::PhysicalSize::new(msize.width, msize.height))
        .map_err(|e| e.to_string())?;
    let _ = win.show();
    let _ = win.set_focus();
    Ok(frame)
}

/// Restore the main widget to its pre-capture size and position.
#[tauri::command]
fn exit_capture(app: tauri::AppHandle, saved: State<'_, SavedBounds>) -> Result<(), String> {
    let win = app.get_webview_window("main").ok_or("no main window")?;
    if let Some((x, y, w, h)) = saved.0.lock().unwrap().take() {
        let _ = win.set_size(tauri::PhysicalSize::new(w, h));
        let _ = win.set_position(tauri::PhysicalPosition::new(x, y));
    }
    let _ = win.set_resizable(false);
    Ok(())
}

/// Return the frozen full-screen frame for the overlay to crop.
#[tauri::command]
fn get_capture_frame(state: State<'_, CaptureState>) -> Option<String> {
    state.0.lock().unwrap().clone()
}

/// Locate the `models` directory that actually contains the speech models.
///
/// In a packaged build the models are bundled under the Resource dir. In dev the
/// Resource dir (`target/debug/models`) exists but is not populated with the
/// large model files, so we fall back to the crate's own `models` folder.
fn resolve_models_dir(app: &tauri::App) -> PathBuf {
    let has_models = |dir: &Path| dir.join("sense-voice").join("model.int8.onnx").exists();
    if let Ok(path) = app
        .path()
        .resolve("models", tauri::path::BaseDirectory::Resource)
    {
        if has_models(&path) {
            return path;
        }
    }
    let dev = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("models");
    if has_models(&dev) {
        return dev;
    }
    PathBuf::from("models")
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(
            tauri_plugin_log::Builder::new()
                .level(log::LevelFilter::Info)
                // Full detail for our own code without noisy dependency logs.
                .level_for("wg_lib", log::LevelFilter::Trace)
                .target(tauri_plugin_log::Target::new(
                    tauri_plugin_log::TargetKind::Stdout,
                ))
                // Persistent, timestamped log in the app-data log dir
                // (e.g. %APPDATA%\.wg\logs\wg.log on Windows).
                .target(tauri_plugin_log::Target::new(
                    tauri_plugin_log::TargetKind::LogDir {
                        file_name: Some("wg".into()),
                    },
                ))
                .max_file_size(5_000_000)
                .build(),
        )
        .plugin(tauri_plugin_opener::init())
        .manage(CopilotState::default())
        .manage(CaptureState::default())
        .manage(SavedBounds::default())
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
            is_recording,
            enter_capture,
            exit_capture,
            get_capture_frame
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
