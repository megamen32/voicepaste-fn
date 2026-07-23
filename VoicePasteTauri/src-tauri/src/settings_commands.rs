use crate::autostart_manager::AutostartManager;
use crate::config::{AppConfig, AppSettings};
use crate::local_transcriber::{self, ModelStatus};
use crate::models::{ActivationMode, HotkeyKind, Language, SttEngine, UiLanguage};
use crate::native_stt::NativeSttService;
use crate::tray::TrayManager;
use serde_json::Value;
use std::path::Path;
use tauri::{AppHandle, Emitter, Manager, Wry};

/// Check platform-specific permissions (microphone, accessibility).
pub fn check_permissions() -> Value {
    #[cfg(target_os = "macos")]
    {
        let mic_granted = std::process::Command::new("swift")
            .args(["-e", "import AVFoundation; print(AVCaptureDevice.authorizationStatus(for: .audio).rawValue)"])
            .output()
            .ok()
            .and_then(|output| String::from_utf8(output.stdout).ok())
            .map(|value| value.trim() == "3")
            .unwrap_or(false);
        let accessibility_granted = std::process::Command::new("swift")
            .args([
                "-e",
                "import ApplicationServices; print(AXIsProcessTrusted() ? 1 : 0)",
            ])
            .output()
            .ok()
            .and_then(|output| String::from_utf8(output.stdout).ok())
            .map(|value| value.trim() == "1")
            .unwrap_or(false);
        serde_json::json!({"microphone": mic_granted, "accessibility": accessibility_granted})
    }
    #[cfg(not(target_os = "macos"))]
    {
        serde_json::json!({"microphone": true, "accessibility": true})
    }
}

#[derive(Debug, serde::Deserialize, Default)]
pub struct SettingsPatch {
    base_url: Option<String>,
    api_key: Option<String>,
    clear_api_key: Option<bool>,
    model: Option<String>,
    local_model: Option<String>,
    local_command: Option<String>,
    remote_provider: Option<String>,
    language: Option<Language>,
    realtime_preview: Option<bool>,
    recording_delay: Option<f64>,
    hide_delay: Option<f64>,
    hotkey: Option<HotkeyKind>,
    activation_mode: Option<ActivationMode>,
    overlay_centered: Option<bool>,
    wake_server_on_start: Option<bool>,
    realtime_chunk_interval: Option<f64>,
    autostart: Option<bool>,
    engine_order: Option<Vec<SttEngine>>,
    ui_language: Option<UiLanguage>,
}

fn local_model_status(config: &AppConfig) -> Value {
    if config.local_model == local_transcriber::LOCAL_MODEL_PARAKEET_V3 {
        let configured = config
            .local_command
            .as_deref()
            .map(|command| !command.trim().is_empty())
            .unwrap_or_else(|| {
                std::env::var("PARAKEET_ASR_COMMAND")
                    .map(|command| !command.trim().is_empty())
                    .unwrap_or(false)
            });
        return serde_json::json!({
            "state": if configured { "command_ready" } else { "command_missing" },
            "path": Value::Null,
            "bytes": 0,
        });
    }

    match local_transcriber::LocalTranscriber::model_status_for(&config.local_model) {
        ModelStatus::NotPresent => serde_json::json!({
            "state": "missing",
            "path": Value::Null,
            "bytes": 0,
        }),
        ModelStatus::Present { path, bytes } => serde_json::json!({
            "state": "ready",
            "path": path,
            "bytes": bytes,
        }),
    }
}

fn detected_proxy_env() -> Vec<&'static str> {
    ["HTTP_PROXY", "HTTPS_PROXY", "ALL_PROXY", "NO_PROXY"]
        .into_iter()
        .filter(|name| std::env::var_os(name).is_some())
        .collect()
}

/// Return settings for the web UI without ever returning the raw API key.
#[tauri::command]
pub fn get_settings() -> Result<Value, String> {
    let config = AppSettings::global().get();
    Ok(serde_json::json!({
        "base_url": config.effective_base_url(),
        "api_key_set": !config.effective_api_key().is_empty(),
        "api_key_masked": config.masked_api_key(),
        "model": config.effective_model(),
        "local_model": config.local_model,
        "local_command": config.local_command,
        "remote_provider": config.remote_provider,
        "language": config.language,
        "realtime_preview": config.realtime_preview,
        "recording_delay": config.recording_delay_clamped(),
        "hide_delay": config.hide_delay_clamped(),
        "hotkey": config.hotkey,
        "activation_mode": config.activation_mode,
        "overlay_centered": config.overlay_centered,
        "wake_server_on_start": config.wake_server_on_start,
        "realtime_chunk_interval": config.realtime_chunk_interval_clamped(),
        "autostart": config.autostart,
        "engine_order": config.engine_order,
        "ui_language": config.effective_ui_language(),
        "models_dir": local_transcriber::models_dir(),
        "config_path": AppSettings::global().config_path(),
        "local_model_status": local_model_status(&config),
        "parakeet_model_url": local_transcriber::PARAKEET_V3_MODEL_URL,
        "proxy_env": detected_proxy_env(),
        "system_proxy_supported": true,
        "permissions": check_permissions(),
        "native_available": NativeSttService::is_available(),
    }))
}

#[tauri::command]
pub fn refresh_remote_models() -> Result<Vec<String>, String> {
    let config = AppSettings::global().get();
    Ok(crate::transcriber::Transcriber::new().fetch_models(&config))
}

/// Apply a partial settings update from the full Settings window.
#[tauri::command]
pub fn save_settings(app: AppHandle<Wry>, patch: SettingsPatch) -> Result<Value, String> {
    let current = AppSettings::global().get();
    if let Some(enabled) = patch.autostart {
        if enabled != current.autostart {
            AutostartManager::set_enabled(enabled)?;
        }
    }

    let language_changed = patch.ui_language;
    let hotkey_changed = patch.hotkey;
    let new_api_key = if patch.clear_api_key.unwrap_or(false) {
        Some(None)
    } else {
        patch.api_key.map(|key| {
            if key.trim().is_empty() {
                None
            } else {
                Some(key)
            }
        })
    };

    AppSettings::global().update(|config| {
        if let Some(value) = patch.base_url {
            config.base_url = value.trim().trim_end_matches('/').to_string();
        }
        if let Some(value) = new_api_key {
            config.api_key = value;
        }
        if let Some(value) = patch.model {
            config.model = value;
        }
        if let Some(value) = patch.local_model {
            config.local_model = value;
        }
        if let Some(value) = patch.local_command {
            config.local_command = if value.trim().is_empty() {
                None
            } else {
                Some(value)
            };
        }
        if let Some(value) = patch.remote_provider {
            config.remote_provider = value;
        }
        if let Some(value) = patch.language {
            config.language = value;
        }
        if let Some(value) = patch.realtime_preview {
            config.realtime_preview = value;
        }
        if let Some(value) = patch.recording_delay {
            config.recording_delay = value.clamp(0.1, 2.0);
        }
        if let Some(value) = patch.hide_delay {
            config.hide_delay = value.clamp(0.0, 5.0);
        }
        if let Some(value) = patch.hotkey {
            config.hotkey = value;
        }
        if let Some(value) = patch.activation_mode {
            config.activation_mode = value;
        }
        if let Some(value) = patch.overlay_centered {
            config.overlay_centered = value;
        }
        if let Some(value) = patch.wake_server_on_start {
            config.wake_server_on_start = value;
        }
        if let Some(value) = patch.realtime_chunk_interval {
            config.realtime_chunk_interval = value.clamp(1.0, 30.0);
        }
        if let Some(value) = patch.autostart {
            config.autostart = value;
        }
        if let Some(value) = patch.engine_order {
            if !value.is_empty() {
                config.engine_order = value;
            }
        }
        if let Some(value) = patch.ui_language {
            config.ui_language = Some(value);
        }
    });

    if let Some(kind) = hotkey_changed {
        let state = app.state::<crate::AppState>();
        let mut hotkey = state.hotkey.lock();
        hotkey.register(&app, kind)?;
    }
    if let Some(language) = language_changed {
        let _ = app.emit("ui-language-changed", language.code());
    }
    TrayManager::new(app.clone()).rebuild();
    get_settings()
}

#[tauri::command]
pub fn open_settings(app: AppHandle<Wry>) -> Result<(), String> {
    let window = app
        .get_webview_window("settings")
        .ok_or_else(|| "Settings window is not configured".to_string())?;
    window.show().map_err(|e| e.to_string())?;
    window.set_focus().map_err(|e| e.to_string())
}

#[tauri::command]
pub fn download_local_model(app: AppHandle<Wry>) -> Result<(), String> {
    let _ = app.emit(
        "local-model-progress",
        serde_json::json!({"state": "starting"}),
    );
    std::thread::spawn(move || {
        let result = local_transcriber::download_default_model(|downloaded, total| {
            let _ = app.emit(
                "local-model-progress",
                serde_json::json!({
                    "state": "downloading", "downloaded": downloaded, "total": total
                }),
            );
        });
        let payload = match result {
            Ok(path) => serde_json::json!({"state": "ready", "path": path}),
            Err(error) => serde_json::json!({"state": "error", "error": error}),
        };
        let _ = app.emit("local-model-progress", payload);
    });
    Ok(())
}

fn open_path(path: &Path) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    let result = std::process::Command::new("open").arg(path).spawn();
    #[cfg(target_os = "windows")]
    let result = std::process::Command::new("explorer").arg(path).spawn();
    #[cfg(target_os = "linux")]
    let result = std::process::Command::new("xdg-open").arg(path).spawn();
    result
        .map(|_| ())
        .map_err(|e| format!("failed to open path: {}", e))
}

#[tauri::command]
pub fn open_models_folder() -> Result<(), String> {
    open_path(&local_transcriber::models_dir())
}

#[tauri::command]
pub fn open_model_page() -> Result<(), String> {
    #[cfg(target_os = "macos")]
    let result = std::process::Command::new("open")
        .arg(local_transcriber::PARAKEET_V3_MODEL_URL)
        .spawn();
    #[cfg(target_os = "windows")]
    let result = std::process::Command::new("cmd")
        .args(["/C", "start", "", local_transcriber::PARAKEET_V3_MODEL_URL])
        .spawn();
    #[cfg(target_os = "linux")]
    let result = std::process::Command::new("xdg-open")
        .arg(local_transcriber::PARAKEET_V3_MODEL_URL)
        .spawn();
    result
        .map(|_| ())
        .map_err(|e| format!("failed to open model page: {}", e))
}

#[tauri::command]
pub fn open_permissions() -> Result<(), String> {
    #[cfg(target_os = "macos")]
    let result = std::process::Command::new("open")
        .arg("x-apple.systempreferences:com.apple.preference.security?Privacy")
        .spawn();
    #[cfg(target_os = "windows")]
    let result = std::process::Command::new("cmd")
        .args(["/C", "start", "", "ms-settings:privacy-microphone"])
        .spawn();
    #[cfg(target_os = "linux")]
    let result = std::process::Command::new("xdg-open")
        .arg("settings://")
        .spawn();
    result
        .map(|_| ())
        .map_err(|e| format!("failed to open permissions: {}", e))
}
