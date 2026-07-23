pub mod audio_recorder;
pub mod autostart_manager;
pub mod config;
pub mod hotkey;
pub mod local_transcriber;
pub mod models;
pub mod native_stt;
pub mod overlay;
pub mod pasteboard_typer;
pub mod recording_queue;
pub mod settings_commands;
pub mod text_cleaner;
pub mod transcriber;
pub mod transcription_service;
pub mod tray;
pub mod tray_events;
pub mod wake_wav;

use audio_recorder::AudioRecorder;
use config::{AppConfig, AppSettings};
use hotkey::HotkeyManager;
use local_transcriber::LocalTranscriber;
use models::{ActivationMode, SttEngine, UiLanguage};
use native_stt::NativeSttService;
use overlay::OverlayManager;
use pasteboard_typer::PasteboardTyper;
use recording_queue::{RecordingAction, RecordingQueueCoordinator};
use std::path::PathBuf;
use tauri::{Emitter, Listener, Manager, Wry};
use text_cleaner::TextCleaner;
use transcription_service::{
    CascadeTranscriber, CommandTranscriptionService, RetryTranscriber,
    ServerTranscriptionService, TranscriptionService,
};
use settings_commands::{
    download_local_model, get_settings, open_model_page, open_models_folder, open_permissions,
    open_settings, refresh_remote_models, save_settings,
};
use tray::TrayManager;
use wake_wav::WakeWav;

/// Path to the rolling log file. Lives under `~/Library/Logs/VoicePaste/`
/// (the standard macOS user-logs location) so `Diagnostics → Open Logs` opens
/// it in Console.app with proper streaming.
pub fn log_path() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_default();
    PathBuf::from(home)
        .join("Library/Logs/VoicePaste")
        .join("VoicePaste.log")
}

/// Tee writer: every write goes to both stderr and the log file.
/// Used by env_logger as a single Target::Pipe sink.
struct TeeWriter {
    file: std::fs::File,
}

impl std::io::Write for TeeWriter {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let _ = std::io::stderr().write_all(buf);
        // Best-effort: if the log file is gone (user deleted it), don't crash
        // the app. Just keep going with stderr.
        let _ = self.file.write_all(buf);
        Ok(buf.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        let _ = std::io::stderr().flush();
        let _ = self.file.flush();
        Ok(())
    }
}

/// Install a logger that writes to BOTH stderr and the log file.
/// Returns the path of the log file (so the caller can log it).
pub fn init_logging() -> PathBuf {
    let path = log_path();
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let file = match std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
    {
        Ok(f) => f,
        Err(e) => {
            eprintln!("could not open log file {}: {} — falling back to stderr only", path.display(), e);
            let _ = env_logger::Builder::from_env(
                env_logger::Env::default().default_filter_or("info"),
            ).try_init();
            return path;
        }
    };
    let writer = TeeWriter { file };
    let _ = env_logger::Builder::from_env(
        env_logger::Env::default().default_filter_or("info"),
    )
    .target(env_logger::Target::Pipe(Box::new(writer)))
    .format(|buf, record| {
        use std::io::Write;
        writeln!(buf, "[{}] [{}] [{}] {}",
            chrono::Utc::now().to_rfc3339(),
            record.level(),
            record.target(),
            record.args())
    })
    .try_init();
    path
}

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
///
/// Kept for back-compat with the existing `RetryTranscriber` test suite; the
/// new canonical factory is `make_cascade_transcriber` (which composes N
/// tiers from `config.engine_order`).
#[allow(dead_code)]
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
        match build_local_engine(config) {
            Some(service) => {
                log::info!("Local fallback enabled with provider {}", config.local_model);
                Some(service)
            }
            None => {
                // SILENT before — this was the actual reason the fallback never
                // fired on 500s. The user thought "Transcription error" meant
                // "server is broken", but local fallback was never wired up
                // because no model file exists yet. Surface this loudly so the
                // user can either toggle the setting off, drop a model in the
                // expected folder, or hit "Download local model" in the tray.
                log::warn!(
                    "local_fallback is ON but no whisper model found. Looked in: \
                     $WHISPER_MODEL_PATH, ~/Library/Application Support/com.bezrabotnyi.voicepaste/ggml-base.bin, \
                     ./ggml-base.bin. Use the tray Diagnostics menu to download or open the models folder."
                );
                None
            }
        }
    } else {
        log::info!("Local fallback disabled (toggle in tray)");
        None
    };

    RetryTranscriber::new(Box::new(server), fallback, 3)
}

fn build_local_engine(config: &AppConfig) -> Option<Box<dyn TranscriptionService>> {
    if config.local_model == local_transcriber::LOCAL_MODEL_PARAKEET_V3 {
        let command = config
            .local_command
            .clone()
            .or_else(|| std::env::var("PARAKEET_ASR_COMMAND").ok())
            .filter(|value| !value.trim().is_empty())?;
        return Some(Box::new(CommandTranscriptionService::new(command)));
    }

    LocalTranscriber::find_model_for(&config.local_model)
        .map(|path| Box::new(LocalTranscriber::new(path)) as Box<dyn TranscriptionService>)
}

/// Is the given engine usable right now? Used by `make_cascade_transcriber`
/// to skip tiers that can't run on this platform / with this configuration.
///
/// - `Remote`: always true (network might still be down, but that's a runtime
///   error the engine itself reports — not an availability question).
/// - `Local`: true iff a whisper.cpp model file is discoverable on disk.
/// - `Native`: true on macOS, false everywhere else.
pub fn is_engine_available(engine: SttEngine) -> bool {
    match engine {
        SttEngine::Remote => true,
        SttEngine::Local => LocalTranscriber::find_model().is_some(),
        SttEngine::Native => NativeSttService::is_available(),
    }
}

/// Construct a single `TranscriptionService` for the given engine, or
/// `None` if the engine is not available right now. Pure factory; no
/// logging, no side effects.
fn build_engine(config: &AppConfig, engine: SttEngine) -> Option<Box<dyn TranscriptionService>> {
    match engine {
        SttEngine::Remote => {
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
            Some(Box::new(server))
        }
        SttEngine::Local => build_local_engine(config),
        SttEngine::Native => {
            if NativeSttService::is_available() {
                let lang = config
                    .language
                    .api_value()
                    .unwrap_or("auto")
                    .to_string();
                Some(Box::new(NativeSttService::new(lang)))
            } else {
                None
            }
        }
    }
}

/// Build a `CascadeTranscriber` from `config.engine_order`. Each engine that
/// is not available right now is skipped (with a warning), so the user can
/// keep `Local` in their `engine_order` even when no model is downloaded —
/// it just won't participate until they drop one in.
///
/// Replaces `make_retry_transcriber` as the canonical factory; the old name
/// is kept for back-compat with any external callers.
fn make_cascade_transcriber(config: &AppConfig) -> CascadeTranscriber {
    let mut tiers: Vec<Box<dyn TranscriptionService>> = Vec::new();

    for engine in &config.engine_order {
        match build_engine(config, *engine) {
            Some(svc) => {
                log::info!("cascade: enabled tier {:?}", engine);
                tiers.push(svc);
            }
            None => {
                log::warn!(
                    "cascade: skipping {:?} — not available (see prior warning)",
                    engine
                );
            }
        }
    }

    CascadeTranscriber::new(tiers)
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

    // Short-recording guard. If the user released the hotkey faster than
    // `MIN_RECORDING_DURATION_S` (typical when tapping Fn), the WAV is
    // either header-only or filled with a handful of silence samples.
    // Don't waste a server round-trip / a whisper.cpp "empty input" error
    // on it — log info and return early. The cascade already stops on
    // empty results, but a real HTTP 500 fallback would still hit the
    // broken Local path; this skips the cascade entirely.
    match audio_recorder::wav_duration_seconds(&audio_path) {
        Ok(d) if d < audio_recorder::MIN_RECORDING_DURATION_S => {
            log::info!(
                "Recording too short ({:.3}s < {:.2}s threshold), skipping transcription",
                d,
                audio_recorder::MIN_RECORDING_DURATION_S
            );
            overlay.hide();
            return Ok(());
        }
        _ => {} // duration is fine (or unreadable — let the cascade surface the real error)
    }

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
        let cascade = make_cascade_transcriber(&settings);
        let lang_code = settings.language.api_value();

        match cascade.transcribe(&audio_path, lang_code) {
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
                overlay.show_retry(&e);
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
        let cascade = make_cascade_transcriber(&settings);
        let lang_code = settings.language.api_value();

        match cascade.transcribe(&audio_path, lang_code) {
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
                overlay.show_retry(&e);
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

/// Initialize the application UI language once, using the webview's actual
/// system locale. This is more reliable on Windows than environment variables.
#[tauri::command]
fn initialize_ui_language(
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
        TrayManager::new(app.clone()).rebuild();
    }
    let _ = app.emit("ui-language-changed", selected.code());
    Ok(selected.code().to_string())
}

/// Show a dialog by resizing the overlay window.
#[tauri::command]
fn show_dialog(app: tauri::AppHandle<Wry>) -> Result<(), String> {
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
fn hide_dialog(app: tauri::AppHandle<Wry>) -> Result<(), String> {
    if let Some(window) = app.get_webview_window("overlay") {
        let _ = window.set_size(tauri::LogicalSize::new(64u32, 44u32));
        let _ = window.hide();
    }
    Ok(())
}

/// Get macOS permission status (microphone, accessibility).
#[tauri::command]
fn get_permissions() -> Result<serde_json::Value, String> {
    Ok(settings_commands::check_permissions())
}

/// Show the record window.
#[tauri::command]
fn show_record_window(app: tauri::AppHandle<Wry>) -> Result<(), String> {
    if let Some(window) = app.get_webview_window("record") {
        let _ = window.set_focus();
        let _ = window.show();
    }
    Ok(())
}

/// Hide the record window.
#[tauri::command]
fn hide_record_window(app: tauri::AppHandle<Wry>) -> Result<(), String> {
    if let Some(window) = app.get_webview_window("record") {
        let _ = window.hide();
    }
    Ok(())
}

/// Start recording mode (for record window).
#[tauri::command]
fn start_record_mode(app: tauri::AppHandle<Wry>) -> Result<serde_json::Value, String> {
    let state = app.state::<AppState>();
    let mut recorder = state.recorder.lock();
    
    match recorder.start() {
        Ok(_) => {
            *state.is_recording.lock() = true;
            Ok(serde_json::json!({"success": true}))
        }
        Err(e) => Ok(serde_json::json!({"success": false, "error": e})),
    }
}

/// Stop recording mode and transcribe (for record window).
#[tauri::command]
fn stop_record_mode(app: tauri::AppHandle<Wry>) -> Result<serde_json::Value, String> {
    let state = app.state::<AppState>();
    let settings = AppSettings::global().get();

    let mut recorder = state.recorder.lock();
    *state.is_recording.lock() = false;

    let path = match recorder.stop() {
        Some(path) => path,
        None => return Ok(serde_json::json!({"success": true, "text": ""})),
    };
    drop(recorder); // release the lock before re-acquiring it for the cascade

    // Same short-recording guard as `stop_and_transcribe` — see comment there.
    if let Ok(d) = audio_recorder::wav_duration_seconds(&path) {
        if d < audio_recorder::MIN_RECORDING_DURATION_S {
            log::info!(
                "Recording too short ({:.3}s < {:.2}s threshold), skipping transcription",
                d,
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

/// Copy text to clipboard.
#[tauri::command]
fn copy_to_clipboard(text: String) -> Result<(), String> {
    let paster = PasteboardTyper::new();
    paster.paste(&text);
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

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let log_file = std::sync::Arc::new(init_logging());

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
        .setup(move |app| {
            let handle: tauri::AppHandle<Wry> = app.handle().clone();

            // Build tray menu
            let tray_manager = TrayManager::new(handle.clone());
            let menu = tray_manager.build_menu().map_err(|e| tauri::Error::Anyhow(anyhow::anyhow!(e)))?;

            // Set up tray icon
            if let Some(tray) = handle.tray_by_id("main-tray") {
                let handle_clone = handle.clone();
                tray.on_menu_event(move |_tray, event| {
                    tray_events::handle_tray_event(&handle_clone, event.id().as_ref());
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
            log::info!("Log file: {}", log_file.display());
            log::info!("Endpoint: {}", settings.effective_base_url());
            log::info!("Hotkey: {}", settings.hotkey.title());
            drop(log_file); // silence unused if any future read happens

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            start_recording,
            stop_and_transcribe,
            retry_transcription,
            save_endpoint,
            save_api_key,
            get_settings,
            save_settings,
            refresh_remote_models,
            open_settings,
            download_local_model,
            open_models_folder,
            open_model_page,
            open_permissions,
            initialize_ui_language,
            show_dialog,
            hide_dialog,
            get_permissions,
            show_record_window,
            hide_record_window,
            start_record_mode,
            stop_record_mode,
            copy_to_clipboard,
        ])
        .run(tauri::generate_context!())
        .expect("error while running VoicePaste");
}

#[cfg(test)]
mod cascade_wiring_tests {
    use super::*;
    use crate::models::SttEngine;

    #[test]
    fn is_engine_available_remote_is_always_true() {
        // Remote has no platform / file prerequisites — it's always usable
        // (network failures are a runtime concern of the engine itself, not
        // an availability check).
        assert!(is_engine_available(SttEngine::Remote));
    }

    #[test]
    fn is_engine_available_native_matches_native_stt() {
        // The lib-level helper and the NativeSttService itself must agree.
        assert_eq!(
            is_engine_available(SttEngine::Native),
            NativeSttService::is_available()
        );
    }

    #[test]
    fn build_engine_remote_always_succeeds() {
        // Remote is always buildable, regardless of config contents.
        let cfg = AppConfig::default();
        let svc = build_engine(&cfg, SttEngine::Remote);
        assert!(svc.is_some(), "Remote engine should always build");
    }

    #[test]
    fn build_engine_local_requires_model_file() {
        // On a clean machine there's no whisper model → Local builds to None.
        // (If a model IS present this returns Some, which is also fine —
        // both branches prove the factory is wired correctly.)
        let cfg = AppConfig::default();
        let svc = build_engine(&cfg, SttEngine::Local);
        let found_model = crate::local_transcriber::LocalTranscriber::find_model().is_some();
        assert_eq!(svc.is_some(), found_model);
    }
}
