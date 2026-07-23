use crate::autostart_manager::AutostartManager;
use crate::config::{AppConfig, AppSettings};
use crate::history;
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
        let accessibility_granted = crate::hotkey::accessibility_granted();
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
    history_retention_days: Option<u32>,
}

fn parakeet_command_configured(config: &AppConfig) -> bool {
    config
        .local_command
        .as_deref()
        .map(|command| !command.trim().is_empty())
        .unwrap_or_else(|| {
            std::env::var("PARAKEET_ASR_COMMAND")
                .map(|command| !command.trim().is_empty())
                .unwrap_or(false)
        })
}

fn local_model_status(config: &AppConfig, model: &str) -> Value {
    let runtime_configured = if model == local_transcriber::LOCAL_MODEL_PARAKEET_V3 {
        parakeet_command_configured(config)
    } else {
        true
    };

    let model_ready = match local_transcriber::LocalTranscriber::model_status_for(model) {
        ModelStatus::NotPresent => serde_json::json!({
            "state": "missing",
            "model_ready": false,
            "runtime_configured": runtime_configured,
            "path": Value::Null,
            "bytes": 0,
        }),
        ModelStatus::Present { path, bytes } => serde_json::json!({
            "state": if runtime_configured { "ready" } else { "runtime_missing" },
            "model_ready": true,
            "runtime_configured": runtime_configured,
            "path": path,
            "bytes": bytes,
        }),
    };
    model_ready
}

fn local_model_is_ready(model: &str) -> bool {
    matches!(
        local_transcriber::LocalTranscriber::model_status_for(model),
        ModelStatus::Present { .. }
    )
}

fn local_engine_available(model: &str, config: &AppConfig) -> bool {
    if !local_model_is_ready(model) {
        return false;
    }
    model != local_transcriber::LOCAL_MODEL_PARAKEET_V3 || parakeet_command_configured(config)
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
        "engine_availability": {
            "remote": true,
            "local": local_engine_available(&config.local_model, &config),
            "native": NativeSttService::is_available(),
        },
        "ui_language": config.effective_ui_language(),
        "history_retention_days": config.history_retention_days,
        "models_dir": local_transcriber::models_dir(),
        "config_path": AppSettings::global().config_path(),
        "local_model_status": local_model_status(&config, &config.local_model),
        "model_statuses": {
            local_transcriber::LOCAL_MODEL_WHISPER_BASE: local_model_status(
                &config,
                local_transcriber::LOCAL_MODEL_WHISPER_BASE,
            ),
            local_transcriber::LOCAL_MODEL_PARAKEET_V3: local_model_status(
                &config,
                local_transcriber::LOCAL_MODEL_PARAKEET_V3,
            ),
        },
        "parakeet_model_url": local_transcriber::PARAKEET_V3_MODEL_URL,
        "parakeet_archive_url": local_transcriber::PARAKEET_V3_MODEL_ARCHIVE_URL,
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
    if let Some(local_model) = patch.local_model.as_deref() {
        if ![
            local_transcriber::LOCAL_MODEL_WHISPER_BASE,
            local_transcriber::LOCAL_MODEL_PARAKEET_V3,
        ]
        .contains(&local_model)
        {
            return Err(format!("Unknown local model: {}", local_model));
        }
        if local_model != current.local_model && !local_model_is_ready(local_model) {
            return Err(format!(
                "Local model '{}' is not downloaded yet. Download it before selecting it.",
                local_model
            ));
        }
    }
    if let Some(engine_order) = patch.engine_order.as_ref() {
        let selected_model = patch
            .local_model
            .as_deref()
            .unwrap_or(current.local_model.as_str());
        let mut candidate = current.clone();
        if let Some(model) = patch.local_model.as_ref() {
            candidate.local_model = model.clone();
        }
        if let Some(command) = patch.local_command.as_ref() {
            candidate.local_command = if command.trim().is_empty() {
                None
            } else {
                Some(command.clone())
            };
        }
        if engine_order.contains(&SttEngine::Local)
            && !local_engine_available(selected_model, &candidate)
        {
            return Err(
                "Local engine is unavailable. Download the selected model and configure its runtime first."
                    .to_string(),
            );
        }
    }
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
        if let Some(value) = patch.history_retention_days {
            config.history_retention_days = value;
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
pub fn download_local_model(app: AppHandle<Wry>, model: Option<String>) -> Result<(), String> {
    let model = model
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| local_transcriber::LOCAL_MODEL_WHISPER_BASE.to_string());
    if ![
        local_transcriber::LOCAL_MODEL_WHISPER_BASE,
        local_transcriber::LOCAL_MODEL_PARAKEET_V3,
    ]
    .contains(&model.as_str())
    {
        return Err(format!("Unknown local model: {}", model));
    }

    let _ = app.emit(
        "local-model-progress",
        serde_json::json!({"state": "starting", "model": model}),
    );
    std::thread::spawn(move || {
        let model_for_event = model.clone();
        let result = if model == local_transcriber::LOCAL_MODEL_PARAKEET_V3 {
            local_transcriber::download_parakeet_model(|downloaded, total| {
                let _ = app.emit(
                    "local-model-progress",
                    serde_json::json!({
                        "state": "downloading", "model": model_for_event.clone(),
                        "downloaded": downloaded, "total": total
                    }),
                );
            })
        } else {
            local_transcriber::download_default_model(|downloaded, total| {
                let _ = app.emit(
                    "local-model-progress",
                    serde_json::json!({
                        "state": "downloading", "model": model_for_event.clone(),
                        "downloaded": downloaded, "total": total
                    }),
                );
            })
        };
        let payload = match result {
            Ok(path) => serde_json::json!({"state": "ready", "model": model, "path": path}),
            Err(error) => serde_json::json!({"state": "error", "model": model, "error": error}),
        };
        let _ = app.emit("local-model-progress", payload);
    });
    Ok(())
}

#[tauri::command]
pub fn get_history() -> Result<Vec<history::HistoryEntry>, String> {
    let config = AppSettings::global().get();
    history::list(config.history_retention_days)
}

#[tauri::command]
pub fn clear_history() -> Result<(), String> {
    history::clear()
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
