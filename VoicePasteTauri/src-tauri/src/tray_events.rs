use crate::autostart_manager::AutostartManager;
use crate::config;
use crate::config::AppSettings;
use crate::local_transcriber;
use crate::local_transcriber::LocalTranscriber;
use crate::models::{ActivationMode, HotkeyKind, Language, UiLanguage};
use crate::settings_commands::{download_local_model, open_models_folder, open_settings};
use crate::tray::TrayManager;
use crate::{log_path, show_record_window, AppState};
use tauri::{Emitter, Manager, Wry};

/// Handle tray menu events.
pub fn handle_tray_event(app: &tauri::AppHandle<Wry>, event: &str) {
    let settings = AppSettings::global();

    match event {
        "edit_endpoint" => {
            let _ = app.emit("dialog-endpoint", ());
        }
        "edit_api_key" => {
            let _ = app.emit("dialog-api-key", ());
        }
        "toggle_realtime" => {
            settings.update(|c| c.realtime_preview = !c.realtime_preview);
            TrayManager::new(app.clone()).rebuild();
        }
        "toggle_autostart" => {
            let new_val = !settings.get().autostart;
            settings.update(|c| c.autostart = new_val);
            let _ = AutostartManager::set_enabled(new_val);
            TrayManager::new(app.clone()).rebuild();
        }
        "toggle_overlay_centered" => {
            settings.update(|c| c.overlay_centered = !c.overlay_centered);
            TrayManager::new(app.clone()).rebuild();
        }
        "toggle_wake_server" => {
            settings.update(|c| c.wake_server_on_start = !c.wake_server_on_start);
            TrayManager::new(app.clone()).rebuild();
        }
        "toggle_local_fallback" => {
            settings.update(|c| c.local_fallback = !c.local_fallback);
            TrayManager::new(app.clone()).rebuild();
        }
        "permissions_info" => {
            // Open macOS System Settings > Privacy & Security
            let _ = std::process::Command::new("open")
                .args(["x-apple.systempreferences:com.apple.preference.security?Privacy"])
                .output();
        }
        "open_record_window" => {
            let _ = show_record_window(app.clone());
        }
        "open_settings" => {
            let _ = open_settings(app.clone());
        }
        "open_logs" => {
            let path = log_path();
            // Ensure file exists so `open` doesn't fail on a fresh install.
            if let Some(parent) = path.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            if !path.exists() {
                let _ = std::fs::write(&path, b"");
            }
            log::info!("Diagnostics: opening logs at {}", path.display());
            // .log extension → Console.app by default on macOS.
            // -a Console ensures we land in the unified log viewer even if the
            // user has set TextEdit as the default opener for .log.
            let _ = std::process::Command::new("open")
                .arg("-a")
                .arg("Console")
                .arg(&path)
                .spawn();
        }
        "reveal_logs" => {
            let path = log_path();
            log::info!("Diagnostics: revealing logs at {}", path.display());
            let _ = std::process::Command::new("open")
                .arg("-R")
                .arg(&path)
                .spawn();
        }
        "open_config" => {
            let path = AppSettings::default_config_path();
            if !path.exists() {
                // Settings haven't been persisted yet (default-only). Force a save.
                let _ = AppSettings::global().get();
            }
            log::info!("Diagnostics: opening config at {}", path.display());
            // .json → default text editor (TextEdit, BBEdit, VSCode…).
            let _ = std::process::Command::new("open").arg(&path).spawn();
        }
        "model_auto" => {
            settings.update(|c| c.model = "whisper-1".to_string());
            TrayManager::new(app.clone()).rebuild();
        }
        e if e.starts_with("model_remote_") => {
            let Some(index) = e
                .strip_prefix("model_remote_")
                .and_then(|value| value.parse::<usize>().ok())
            else {
                log::warn!("Invalid remote model menu item: {}", e);
                return;
            };
            let Some(model) = settings.get().remote_models.get(index).cloned() else {
                log::warn!("Remote model menu item is no longer in the catalog: {}", e);
                return;
            };
            settings.update(|c| c.model = model);
            TrayManager::new(app.clone()).rebuild();
        }
        "model_local_whisper" => {
            if !matches!(
                LocalTranscriber::model_status_for(local_transcriber::LOCAL_MODEL_WHISPER_BASE),
                local_transcriber::ModelStatus::Present { .. }
            ) {
                log::warn!("Refusing to select Whisper: model is not downloaded");
                return;
            }
            settings.update(|c| {
                c.local_model = local_transcriber::LOCAL_MODEL_WHISPER_BASE.to_string();
            });
            TrayManager::new(app.clone()).rebuild();
        }
        "model_local_parakeet" => {
            if !matches!(
                LocalTranscriber::model_status_for(local_transcriber::LOCAL_MODEL_PARAKEET_V3),
                local_transcriber::ModelStatus::Present { .. }
            ) {
                log::warn!("Refusing to select Parakeet: model is not downloaded");
                return;
            }
            settings.update(|c| {
                c.local_model = local_transcriber::LOCAL_MODEL_PARAKEET_V3.to_string();
            });
            TrayManager::new(app.clone()).rebuild();
        }
        "model_refresh" => {
            let _ = open_settings(app.clone());
        }
        "quit" => {
            app.exit(0);
        }
        e if e.starts_with("rec_delay_") => {
            if let Some(val_str) = e.strip_prefix("rec_delay_") {
                if let Ok(val) = val_str.parse::<f64>() {
                    settings.update(|c| c.recording_delay = val);
                    TrayManager::new(app.clone()).rebuild();
                }
            }
        }
        e if e.starts_with("hide_delay_") => {
            if let Some(val_str) = e.strip_prefix("hide_delay_") {
                if let Ok(val) = val_str.parse::<f64>() {
                    settings.update(|c| c.hide_delay = val);
                    TrayManager::new(app.clone()).rebuild();
                }
            }
        }
        e if e.starts_with("realtime_cadence_") => {
            if let Some(val_str) = e.strip_prefix("realtime_cadence_") {
                if let Ok(val) = val_str.parse::<f64>() {
                    settings.update(|c| c.realtime_chunk_interval = val);
                    TrayManager::new(app.clone()).rebuild();
                }
            }
        }
        e if e.starts_with("lang_") => {
            let code = e.strip_prefix("lang_").unwrap_or("");
            let lang = match code {
                "ru" => Language::Ru,
                "en" => Language::En,
                "zh" => Language::Zh,
                _ => Language::Auto,
            };
            settings.update(|c| c.language = lang);
            TrayManager::new(app.clone()).rebuild();
        }
        e if e.starts_with("app_lang_") => {
            let code = e.strip_prefix("app_lang_").unwrap_or("");
            let language = match code {
                "ru" => UiLanguage::Ru,
                "zh" => UiLanguage::Zh,
                _ => UiLanguage::En,
            };
            settings.update(|c| c.ui_language = Some(language));
            TrayManager::new(app.clone()).rebuild();
            let _ = app.emit("ui-language-changed", language.code());
        }
        e if e.starts_with("hotkey_") => {
            let key_str = e.strip_prefix("hotkey_").unwrap_or("");
            let kind = match key_str {
                "fn" => HotkeyKind::Fn,
                "right_option" => HotkeyKind::RightOption,
                "right_control" => HotkeyKind::RightControl,
                "right_command" => HotkeyKind::RightCommand,
                "right_shift" => HotkeyKind::RightShift,
                "caps_lock" => HotkeyKind::CapsLock,
                "f13" => HotkeyKind::F13,
                "f14" => HotkeyKind::F14,
                "f15" => HotkeyKind::F15,
                _ => HotkeyKind::Fn,
            };
            settings.update(|c| c.hotkey = kind);
            let st = app.state::<AppState>();
            let mut hotkey = st.hotkey.lock();
            if let Err(error) = hotkey.register(app, kind) {
                log::error!("Hotkey registration failed: {}", error);
                let _ = app.emit("hotkey-error", error);
            }
            TrayManager::new(app.clone()).rebuild();
        }
        e if e.starts_with("activation_") => {
            let mode = match e {
                "activation_hold" => ActivationMode::Hold,
                "activation_toggle" => ActivationMode::Toggle,
                _ => return,
            };
            settings.update(|c| c.activation_mode = mode);
            TrayManager::new(app.clone()).rebuild();
        }
        "engine_remote" | "engine_local" | "engine_native" => {
            use crate::models::SttEngine;
            let target = match event {
                "engine_remote" => SttEngine::Remote,
                "engine_local" => SttEngine::Local,
                "engine_native" => SttEngine::Native,
                _ => unreachable!(),
            };
            // Refuse toggles on engines that can't actually run, so the
            // persisted cascade doesn't include a permanently-broken engine.
            let available = match target {
                SttEngine::Remote => true,
                SttEngine::Local => {
                    let config = settings.get();
                    if config.local_model == local_transcriber::LOCAL_MODEL_PARAKEET_V3 {
                        config
                            .local_command
                            .as_deref()
                            .map(|command| !command.trim().is_empty())
                            .unwrap_or_else(|| std::env::var("PARAKEET_ASR_COMMAND").is_ok())
                            && local_transcriber::find_parakeet_model_dir().is_some()
                    } else {
                        LocalTranscriber::find_model_for(&config.local_model).is_some()
                    }
                }
                SttEngine::Native => cfg!(target_os = "macos"),
            };
            if !available {
                log::warn!(
                    "Refusing to toggle {}: not available in this environment",
                    target.short()
                );
                TrayManager::new(app.clone()).rebuild();
                return;
            }
            let current = settings.get().engine_order;
            let next = config::toggle_engine(&current, target);
            if next == current {
                // toggle_engine refused (last engine) — surface a warning and rebuild
                // to clear the visual state of the click.
                log::warn!(
                    "Refusing to disable the last engine (current: {:?})",
                    current
                );
                TrayManager::new(app.clone()).rebuild();
                return;
            }
            settings.update(|c| c.engine_order = next);
            log::info!("Engine order: {:?}", settings.get().engine_order);
            TrayManager::new(app.clone()).rebuild();
        }
        "download_local_model" => {
            let _ = download_local_model(app.clone(), None);
        }
        "open_models_folder" => {
            let _ = open_models_folder();
        }
        _ => {}
    }
}
