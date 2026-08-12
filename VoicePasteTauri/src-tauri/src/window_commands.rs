//! Tauri commands for the overlay and dedicated record window.

use crate::audio_recorder;
use crate::config::AppSettings;
use crate::models::UiLanguage;
use crate::pasteboard_typer::PasteboardTyper;
use crate::text_cleaner::TextCleaner;
use crate::{make_cascade_transcriber, AppState};
use tauri::{Emitter, Manager, Wry};

/// Show a dialog by resizing the overlay window.
#[tauri::command]
pub fn show_dialog(app: tauri::AppHandle<Wry>) -> Result<(), String> {
    if let Some(window) = app.get_webview_window("overlay") {
        let _ = window.set_size(tauri::LogicalSize::new(360u32, 200u32));
        let _ = window.set_always_on_top(true);
        let _ = window.set_focus();
        let _ = window.show();
    }
    Ok(())
}

/// Hide a dialog by restoring the overlay window to its small size.
#[tauri::command]
pub fn hide_dialog(app: tauri::AppHandle<Wry>) -> Result<(), String> {
    if let Some(window) = app.get_webview_window("overlay") {
        let _ = window.set_size(tauri::LogicalSize::new(64u32, 44u32));
        let _ = window.hide();
    }
    Ok(())
}

#[tauri::command]
pub fn get_permissions() -> Result<serde_json::Value, String> {
    Ok(crate::settings_commands::check_permissions())
}

#[tauri::command]
pub fn initialize_ui_language(
    app: tauri::AppHandle<Wry>,
    locale: Option<String>,
) -> Result<String, String> {
    let settings = AppSettings::global();
    let current = settings.get().ui_language;
    let selected = current.unwrap_or_else(|| {
        locale
            .as_deref()
            .filter(|value| !value.is_empty())
            .map(UiLanguage::from_locale)
            .unwrap_or_else(UiLanguage::system)
    });

    if current.is_none() {
        settings.update(|config| config.ui_language = Some(selected));
        crate::tray::TrayManager::new(app.clone()).rebuild();
    }
    let _ = app.emit("ui-language-changed", selected.code());
    Ok(selected.code().to_string())
}

#[tauri::command]
pub fn show_record_window(app: tauri::AppHandle<Wry>) -> Result<(), String> {
    if let Some(window) = app.get_webview_window("record") {
        let _ = window.set_focus();
        let _ = window.show();
    }
    Ok(())
}

#[tauri::command]
pub fn hide_record_window(app: tauri::AppHandle<Wry>) -> Result<(), String> {
    if let Some(window) = app.get_webview_window("record") {
        let _ = window.hide();
    }
    Ok(())
}

#[tauri::command]
pub fn start_record_mode(app: tauri::AppHandle<Wry>) -> Result<serde_json::Value, String> {
    let state = app.state::<AppState>();
    *state.paste_target_pid.lock() = crate::pasteboard_typer::frontmost_process_id();
    let mut recorder = state.recorder.lock();

    match recorder.start() {
        Ok(_) => {
            *state.is_recording.lock() = true;
            state.queue.lock().on_recording_started();
            Ok(serde_json::json!({"success": true}))
        }
        Err(e) => Ok(serde_json::json!({"success": false, "error": e})),
    }
}

#[tauri::command]
pub fn stop_record_mode(app: tauri::AppHandle<Wry>) -> Result<serde_json::Value, String> {
    let state = app.state::<AppState>();
    let settings = AppSettings::global().get();

    let mut recorder = state.recorder.lock();
    *state.is_recording.lock() = false;

    let path = match recorder.stop() {
        Some(path) => path,
        None => return Ok(serde_json::json!({"success": true, "text": ""})),
    };
    drop(recorder);

    if let Ok(duration) = audio_recorder::wav_duration_seconds(&path) {
        if duration < audio_recorder::MIN_RECORDING_DURATION_S {
            log::info!(
                "Recording too short ({:.3}s < {:.2}s threshold), skipping transcription",
                duration,
                audio_recorder::MIN_RECORDING_DURATION_S
            );
            return Ok(serde_json::json!({"success": true, "text": ""}));
        }
    }

    let cascade = make_cascade_transcriber(&settings);
    let lang_code = settings.language.api_value();
    match cascade.transcribe(&path, lang_code) {
        Ok(text) => {
            let cleaned = TextCleaner::clean(&text);
            let _ = app.emit("record-transcript", &cleaned);
            Ok(serde_json::json!({"success": true, "text": cleaned}))
        }
        Err(e) => Ok(serde_json::json!({"success": false, "error": e})),
    }
}

#[tauri::command]
pub fn copy_to_clipboard(text: String) -> Result<(), String> {
    PasteboardTyper::new().paste(&text)
}
