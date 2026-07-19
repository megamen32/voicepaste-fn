pub mod audio_recorder;
pub mod autostart_manager;
pub mod config;
pub mod hotkey;
pub mod local_transcriber;
pub mod models;
pub mod overlay;
pub mod pasteboard_typer;
pub mod recording_queue;
pub mod text_cleaner;
pub mod transcriber;
pub mod transcription_service;
pub mod tray;
pub mod wake_wav;

use audio_recorder::AudioRecorder;
use autostart_manager::AutostartManager;
use config::{AppConfig, AppSettings};
use hotkey::HotkeyManager;
use local_transcriber::LocalTranscriber;
use models::{ActivationMode, HotkeyKind, Language};
use overlay::OverlayManager;
use pasteboard_typer::PasteboardTyper;
use recording_queue::{RecordingAction, RecordingQueueCoordinator};
use std::path::PathBuf;
use tauri::{Emitter, Listener, Manager, Wry};
use transcription_service::{RetryTranscriber, ServerTranscriptionService, TranscriptionService};
use tray::TrayManager;
use wake_wav::WakeWav;

/// Shared application state managed by Tauri.
pub struct AppState {
    pub recorder: parking_lot::Mutex<AudioRecorder>,
    pub queue: parking_lot::Mutex<RecordingQueueCoordinator>,
    pub hotkey: parking_lot::Mutex<HotkeyManager>,
    pub wake_wav: parking_lot::Mutex<WakeWav>,
    pub preview_text: parking_lot::Mutex<String>,
    pub last_failed_audio: parking_lot::Mutex<Option<PathBuf>>,
    pub is_recording: parking_lot::Mutex<bool>,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            recorder: parking_lot::Mutex::new(AudioRecorder::new()),
            queue: parking_lot::Mutex::new(RecordingQueueCoordinator::new()),
            hotkey: parking_lot::Mutex::new(HotkeyManager::new()),
            wake_wav: parking_lot::Mutex::new(WakeWav::new()),
            preview_text: parking_lot::Mutex::new(String::new()),
            last_failed_audio: parking_lot::Mutex::new(None),
            is_recording: parking_lot::Mutex::new(false),
        }
    }
}

/// Build a RetryTranscriber with current settings.
fn make_retry_transcriber(config: &AppConfig) -> RetryTranscriber {
    let transcriber = transcriber::Transcriber::new();
    let model = if config.effective_model() == "auto" {
        None
    } else {
        Some(config.effective_model())
    };
    let server = ServerTranscriptionService::new(
        transcriber,
        config.clone(),
        config.language,
        model,
    );

    let fallback: Option<Box<dyn TranscriptionService>> = if config.local_fallback {
        LocalTranscriber::find_model().map(|path| {
            Box::new(LocalTranscriber::new(path)) as Box<dyn TranscriptionService>
        })
    } else {
        None
    };

    RetryTranscriber::new(Box::new(server), fallback, 3)
}

/// Start recording audio.
#[tauri::command]
fn start_recording(app: tauri::AppHandle<Wry>) -> Result<(), String> {
    let state = app.state::<AppState>();
    let settings = AppSettings::global().get();

    // Wake server if enabled
    if settings.wake_server_on_start {
        let mut wake = state.wake_wav.lock();
        if let Ok(wav_path) = wake.ensure_silence_wav() {
            let transcriber = transcriber::Transcriber::new();
            let _ = transcriber.transcribe(&wav_path, settings.language, None, &settings);
        }
    }

    let mut recorder = state.recorder.lock();
    recorder.start()?;
    *state.is_recording.lock() = true;

    let overlay = OverlayManager::new(app.clone());
    overlay.show_recording();
    overlay.position_near_cursor(settings.overlay_centered);

    Ok(())
}

/// Stop recording and transcribe.
#[tauri::command]
fn stop_and_transcribe(app: tauri::AppHandle<Wry>) -> Result<(), String> {
    let state = app.state::<AppState>();
    let settings = AppSettings::global().get();
    let overlay = OverlayManager::new(app.clone());

    // Stop recording
    let audio_path = {
        let mut recorder = state.recorder.lock();
        *state.is_recording.lock() = false;
        match recorder.stop() {
            Some(path) => path,
            None => {
                overlay.hide();
                return Ok(());
            }
        }
    };

    // Update queue state
    {
        let mut queue = state.queue.lock();
        queue.on_recording_stopped();
    }

    overlay.show_waiting();
    overlay.position_near_cursor(settings.overlay_centered);

    let preview = state.preview_text.lock().clone();

    // Transcribe in background
    let app_clone = app.clone();
    std::thread::spawn(move || {
        let retry = make_retry_transcriber(&settings);
        let lang_code = settings.language.api_value();

        match retry.transcribe(&audio_path, lang_code) {
            Ok(raw_text) => {
                let cleaned = text_cleaner::TextCleaner::clean(&raw_text);
                let result = if cleaned.is_empty() { preview } else { cleaned };

                log::info!("Transcription result: {}", result);
                save_to_ring_buffer(&audio_path);

                let overlay = OverlayManager::new(app_clone.clone());
                overlay.show_preview(&result);
                overlay.position_near_cursor(settings.overlay_centered);

                let typer = PasteboardTyper::new();
                typer.paste(&result);

                let hide_delay = settings.hide_delay_clamped();
                if hide_delay > 0.0 {
                    std::thread::sleep(std::time::Duration::from_secs_f64(hide_delay));
                    overlay.hide();
                }

                let st = app_clone.state::<AppState>();
                let mut queue = st.queue.lock();
                let action = queue.on_transcription_completed();
                if action == RecordingAction::StartRecording {
                    drop(queue);
                    let _ = start_recording(app_clone);
                }
            }
            Err(e) => {
                log::error!("Transcription error: {}", e);
                let st = app_clone.state::<AppState>();
                let retry_path = save_to_ring_buffer(&audio_path);
                st.last_failed_audio.lock().replace(retry_path);

                let overlay = OverlayManager::new(app_clone.clone());
                overlay.show_retry();
                overlay.position_near_cursor(settings.overlay_centered);

                let mut queue = st.queue.lock();
                queue.cancel_pending();
                let action = queue.on_transcription_completed();
                if action == RecordingAction::StartRecording {
                    drop(queue);
                    let _ = start_recording(app_clone);
                }
            }
        }
    });

    Ok(())
}

/// Retry transcription of the last failed audio.
#[tauri::command]
fn retry_transcription(app: tauri::AppHandle<Wry>) -> Result<(), String> {
    let state = app.state::<AppState>();
    let audio_path = state
        .last_failed_audio
        .lock()
        .clone()
        .ok_or("No failed audio to retry")?;

    let settings = AppSettings::global().get();
    let overlay = OverlayManager::new(app.clone());

    {
        let mut queue = state.queue.lock();
        queue.on_retry_started();
    }

    overlay.show_waiting();

    let app_clone = app.clone();
    std::thread::spawn(move || {
        let retry = make_retry_transcriber(&settings);
        let lang_code = settings.language.api_value();

        match retry.transcribe(&audio_path, lang_code) {
            Ok(raw_text) => {
                let cleaned = text_cleaner::TextCleaner::clean(&raw_text);
                log::info!("Retry result: {}", cleaned);

                let _ = std::fs::remove_file(&audio_path);
                let st = app_clone.state::<AppState>();
                st.last_failed_audio.lock().take();

                let overlay = OverlayManager::new(app_clone.clone());
                if !cleaned.is_empty() {
                    overlay.show_preview(&cleaned);
                    let typer = PasteboardTyper::new();
                    typer.paste(&cleaned);
                }

                let hide_delay = settings.hide_delay_clamped();
                if hide_delay > 0.0 {
                    std::thread::sleep(std::time::Duration::from_secs_f64(hide_delay));
                    overlay.hide();
                }

                let mut queue = st.queue.lock();
                let action = queue.on_transcription_completed();
                if action == RecordingAction::StartRecording {
                    drop(queue);
                    let _ = start_recording(app_clone);
                }
            }
            Err(e) => {
                log::error!("Retry error: {}", e);
                let overlay = OverlayManager::new(app_clone.clone());
                overlay.show_retry();
                let st = app_clone.state::<AppState>();
                let mut queue = st.queue.lock();
                queue.clear_busy_after_retry();
            }
        }
    });

    Ok(())
}

/// Save endpoint URL setting.
#[tauri::command]
fn save_endpoint(app: tauri::AppHandle<Wry>, url: String) -> Result<(), String> {
    let settings = AppSettings::global();
    settings.update(|c| c.base_url = url);
    TrayManager::new(app).rebuild();
    Ok(())
}

/// Save API key setting.
#[tauri::command]
fn save_api_key(app: tauri::AppHandle<Wry>, key: String) -> Result<(), String> {
    let settings = AppSettings::global();
    settings.update(|c| c.api_key = if key.is_empty() { None } else { Some(key) });
    TrayManager::new(app).rebuild();
    Ok(())
}

/// Copy audio to ring buffer for retry.
fn save_to_ring_buffer(source: &PathBuf) -> PathBuf {
    let dir = std::env::temp_dir().join("voicepaste-ring");
    let _ = std::fs::create_dir_all(&dir);
    let ts = chrono::Utc::now().timestamp_millis();
    let dest = dir.join(format!("{}.wav", ts));
    let _ = std::fs::copy(source, &dest);
    dest
}

/// Handle tray menu events.
fn handle_tray_event(app: &tauri::AppHandle<Wry>, event: &str) {
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
            let _ = app.emit("dialog-permissions", ());
        }
        "model_auto" => {
            settings.update(|c| c.model = "whisper-1".to_string());
            TrayManager::new(app.clone()).rebuild();
        }
        "model_refresh" => {
            std::thread::spawn(move || {
                let config = settings.get();
                let transcriber = transcriber::Transcriber::new();
                let models = transcriber.fetch_models(&config);
                log::info!("Fetched {} models", models.len());
            });
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
                _ => Language::Auto,
            };
            settings.update(|c| c.language = lang);
            TrayManager::new(app.clone()).rebuild();
        }
        e if e.starts_with("hotkey_") => {
            let key_str = e.strip_prefix("hotkey_").unwrap_or("");
            let kind = match key_str {
                "AltRight" => HotkeyKind::RightAlt,
                "ScrollLock" => HotkeyKind::ScrollLock,
                "CapsLock" => HotkeyKind::CapsLock,
                "Insert" => HotkeyKind::Insert,
                "ShiftRight" => HotkeyKind::RightShift,
                "ControlRight" => HotkeyKind::RightControl,
                _ => HotkeyKind::RightAlt,
            };
            settings.update(|c| c.hotkey = kind);
            let st = app.state::<AppState>();
            let mut hotkey = st.hotkey.lock();
            let _ = hotkey.register(app, kind);
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
        _ => {}
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    env_logger::init();

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .plugin(tauri_plugin_single_instance::init(|_app, _args, _cwd| {
            log::info!("Single instance: app already running");
        }))
        .manage(AppState::new())
        .on_window_event(|_window, event| {
            if let tauri::WindowEvent::Focused(false) = event {
                // Don't steal focus from other apps
            }
        })
        .setup(|app| {
            let handle: tauri::AppHandle<Wry> = app.handle().clone();

            // Build tray menu
            let tray_manager = TrayManager::new(handle.clone());
            let menu = tray_manager.build_menu().map_err(|e| tauri::Error::Anyhow(anyhow::anyhow!(e)))?;

            // Set up tray icon
            if let Some(tray) = handle.tray_by_id("main-tray") {
                let handle_clone = handle.clone();
                tray.on_menu_event(move |_tray, event| {
                    handle_tray_event(&handle_clone, event.id().as_ref());
                });
                let _ = tray.set_menu(Some(menu));
                // Keep tray icon visible
                let _ = tray.set_visible(true);
            }

            // Register global hotkey
            let settings = AppSettings::global().get();
            let state = app.state::<AppState>();
            let mut hotkey = state.hotkey.lock();
            let _ = hotkey.register(&handle, settings.hotkey);

            // Set up hotkey event handling (hold vs toggle)
            let hotkey_handle = handle.clone();
            app.listen("hotkey-pressed", move |_event| {
                let st = hotkey_handle.state::<AppState>();
                let settings = AppSettings::global().get();
                let is_rec = *st.is_recording.lock();

                match settings.activation_mode {
                    ActivationMode::Hold => {
                        if !is_rec {
                            let _ = start_recording(hotkey_handle.clone());
                        }
                    }
                    ActivationMode::Toggle => {
                        if is_rec {
                            let _ = stop_and_transcribe(hotkey_handle.clone());
                        } else {
                            let _ = start_recording(hotkey_handle.clone());
                        }
                    }
                }
            });

            let hotkey_handle2 = handle.clone();
            app.listen("hotkey-released", move |_event| {
                let st = hotkey_handle2.state::<AppState>();
                let settings = AppSettings::global().get();

                if settings.activation_mode == ActivationMode::Hold {
                    let is_rec = *st.is_recording.lock();
                    if is_rec {
                        let _ = stop_and_transcribe(hotkey_handle2.clone());
                    }
                }
            });

            log::info!("VoicePaste started");
            log::info!("Endpoint: {}", settings.effective_base_url());
            log::info!("Hotkey: {}", settings.hotkey.title());

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            start_recording,
            stop_and_transcribe,
            retry_transcription,
            save_endpoint,
            save_api_key,
        ])
        .run(tauri::generate_context!())
        .expect("error while running VoicePaste");
}
