pub mod app_state;
pub mod audio_recorder;
pub mod automation;
pub mod autostart_manager;
pub mod config;
pub mod history;
pub mod hotkey;
pub mod live_preview;
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
pub mod window_commands;

pub use app_state::AppState;
use automation::{AutomationConfig, AutomationPayload, TranscriptDelivery};
use config::{AppConfig, AppSettings};
use local_transcriber::LocalTranscriber;
use models::{ActivationMode, SttEngine};
use native_stt::NativeSttService;
use overlay::OverlayManager;
use pasteboard_typer::PasteboardTyper;
use recording_queue::RecordingAction;
use settings_commands::{
    clear_history, download_local_model, get_history, get_settings, open_model_page,
    open_models_folder, open_permissions, open_settings, refresh_remote_models,
    request_permissions, save_settings,
};
use std::path::PathBuf;
use std::sync::atomic::Ordering;
use tauri::{Emitter, Listener, Manager, Wry};
use transcription_service::{
    CascadeTranscriber, CommandTranscriptionService, RetryTranscriber, ServerTranscriptionService,
    TranscriptionService,
};
use tray::TrayManager;
use window_commands::{
    copy_to_clipboard, get_permissions, hide_dialog, hide_record_window, initialize_ui_language,
    show_dialog, show_record_window, start_record_mode, stop_record_mode,
};

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
            eprintln!(
                "could not open log file {}: {} — falling back to stderr only",
                path.display(),
                e
            );
            let _ =
                env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info"))
                    .try_init();
            return path;
        }
    };
    let writer = TeeWriter { file };
    let _ = env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info"))
        .target(env_logger::Target::Pipe(Box::new(writer)))
        .format(|buf, record| {
            use std::io::Write;
            writeln!(
                buf,
                "[{}] [{}] [{}] {}",
                chrono::Utc::now().to_rfc3339(),
                record.level(),
                record.target(),
                record.args()
            )
        })
        .try_init();
    path
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
    let server =
        ServerTranscriptionService::new(transcriber, config.clone(), config.language, model);

    let fallback: Option<Box<dyn TranscriptionService>> = if config.local_fallback {
        match build_local_engine(config) {
            Some(service) => {
                log::info!(
                    "Local fallback enabled with provider {}",
                    config.local_model
                );
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
        let model_dir = local_transcriber::find_parakeet_model_dir()?;
        let command = config
            .local_command
            .clone()
            .or_else(|| std::env::var("PARAKEET_ASR_COMMAND").ok())
            .filter(|value| !value.trim().is_empty())?;
        return Some(Box::new(
            CommandTranscriptionService::new(command).with_model_dir(model_dir),
        ));
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
                let lang = config.language.api_value().unwrap_or("auto").to_string();
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
pub(crate) fn make_cascade_transcriber(config: &AppConfig) -> CascadeTranscriber {
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

/// Parse the retry choice supplied by the tiny overlay without changing the
/// user's saved cascade order. `None` remains useful to compatibility callers
/// and means retry the normal configured cascade.
fn retry_engine_from_ui(engine: Option<&str>) -> Result<Option<SttEngine>, String> {
    match engine.map(str::trim).filter(|value| !value.is_empty()) {
        None => Ok(None),
        Some("remote") => Ok(Some(SttEngine::Remote)),
        Some("native") => Ok(Some(SttEngine::Native)),
        Some("local") => Ok(Some(SttEngine::Local)),
        Some(value) => Err(format!(
            "Unknown retry method '{}'. Choose Remote, Native, or Local.",
            value
        )),
    }
}

/// Start recording audio.
#[tauri::command]
fn start_recording(app: tauri::AppHandle<Wry>) -> Result<(), String> {
    start_recording_with_delivery(app, TranscriptDelivery::Paste)
}

fn start_recording_with_delivery(
    app: tauri::AppHandle<Wry>,
    delivery: TranscriptDelivery,
) -> Result<(), String> {
    let state = app.state::<AppState>();
    let settings = AppSettings::global().get();
    let test_target_pid = std::env::var("VOICEPASTE_TEST_TARGET_PID")
        .ok()
        .and_then(|raw| raw.parse::<i32>().ok());
    *state.paste_target_pid.lock() =
        test_target_pid.or_else(pasteboard_typer::frontmost_process_id);
    *state.transcript_delivery.lock() = delivery;

    // Wake server if enabled
    if settings.wake_server_on_start {
        let wav_path = {
            let mut wake = state.wake_wav.lock();
            wake.ensure_silence_wav().ok()
        };
        if let Some(wav_path) = wav_path {
            let wake_settings = settings.clone();
            std::thread::spawn(move || {
                let transcriber = transcriber::Transcriber::new();
                let _ =
                    transcriber.transcribe(&wav_path, wake_settings.language, None, &wake_settings);
            });
        }
    }

    let mut recorder = state.recorder.lock();
    recorder.start()?;
    let recording_path = recorder.current_path().cloned();
    let preview_spec = recorder.preview_spec();
    *state.preview_text.lock() = String::new();
    let preview_session = state.preview_session.fetch_add(1, Ordering::SeqCst) + 1;
    let preview_runtime = if settings.realtime_preview {
        preview_spec
            .map(|spec| live_preview::LivePreviewRuntime::new(preview_session, spec, &settings))
    } else {
        None
    };
    drop(recorder);
    // Never nest recorder and preview-runtime locks. The VAD worker and stop
    // path deliberately use preview-runtime -> recorder, so startup installs
    // the runtime only after releasing the recorder.
    *state.preview_runtime.lock() = preview_runtime;
    *state.is_recording.lock() = true;
    state.queue.lock().on_recording_started();

    let overlay = OverlayManager::new(app.clone());
    overlay.show_recording(settings.overlay_centered);
    if let Some(recording_path) = recording_path {
        live_preview::start(
            app.clone(),
            settings.clone(),
            recording_path,
            preview_session,
        );
    }

    Ok(())
}

/// Stop recording and transcribe.
#[tauri::command]
fn stop_and_transcribe(app: tauri::AppHandle<Wry>) -> Result<(), String> {
    let state = app.state::<AppState>();
    let settings = AppSettings::global().get();
    let overlay = OverlayManager::new(app.clone());
    // Stop recording
    let (recorded_audio_path, final_preview_chunks, preview_session) = {
        let mut preview_runtime = state.preview_runtime.lock();
        let mut recorder = state.recorder.lock();
        *state.is_recording.lock() = false;
        let session = preview_runtime
            .as_ref()
            .map(live_preview::LivePreviewRuntime::session)
            .unwrap_or_else(|| state.preview_session.load(Ordering::SeqCst));
        let chunks = if let Some(runtime) = preview_runtime.as_mut() {
            let mut chunks = recorder
                .preview_snapshot_since(runtime.cursor())
                .map(|snapshot| runtime.push(snapshot))
                .unwrap_or_default();
            chunks.extend(runtime.finish());
            chunks
        } else {
            Vec::new()
        };
        let path = match recorder.stop() {
            Some(path) => path,
            None => {
                *preview_runtime = None;
                state.preview_session.fetch_add(1, Ordering::SeqCst);
                overlay.hide();
                return Ok(());
            }
        };
        *preview_runtime = None;
        (path, chunks, session)
    };

    // Deterministic end-to-end canaries may provide a fixture WAV while still
    // exercising the real hotkey, recorder lifecycle, cascade and paste path.
    // This is intentionally opt-in and never set by production launchers.
    let audio_path = match std::env::var("VOICEPASTE_TEST_AUDIO") {
        Ok(path) if !path.trim().is_empty() && std::path::Path::new(&path).is_file() => {
            log::warn!("using VOICEPASTE_TEST_AUDIO fixture for this process");
            PathBuf::from(path)
        }
        _ => recorded_audio_path,
    };

    state.queue.lock().on_recording_stopped();

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
            // Invalidate any preview request that raced with the tap so it
            // cannot update the clipboard after this recording was discarded.
            state.preview_session.fetch_add(1, Ordering::SeqCst);
            overlay.hide();
            return Ok(());
        }
        _ => {} // duration is fine (or unreadable — let the cascade surface the real error)
    }

    overlay.show_waiting();
    overlay.position_near_cursor(settings.overlay_centered);

    let paste_target_pid = *state.paste_target_pid.lock();
    let transcript_delivery = *state.transcript_delivery.lock();
    log::info!(
        "Paste target PID captured before transcription: {:?}",
        paste_target_pid
    );

    // Transcribe in background
    let app_clone = app.clone();
    std::thread::spawn(move || {
        // Finish every incremental phrase first. It may update the clipboard,
        // but never inserts text into the target application.
        live_preview::finish_pending(&app_clone, &settings, preview_session, final_preview_chunks);
        app_clone
            .state::<AppState>()
            .preview_session
            .fetch_add(1, Ordering::SeqCst);

        let cascade = make_cascade_transcriber(&settings);
        let lang_code = settings.language.api_value();

        match cascade.transcribe(&audio_path, lang_code) {
            Ok(raw_text) => {
                // Only a fresh, complete-file result may reach the target app.
                // Never fall back to a concatenated preview draft here.
                let result = text_cleaner::TextCleaner::clean(&raw_text);

                log::info!("Transcription result: {}", result);
                save_to_ring_buffer(&audio_path);
                let duration_ms = audio_recorder::wav_duration_seconds(&audio_path)
                    .ok()
                    .map(|seconds| (seconds * 1000.0) as u64)
                    .unwrap_or_default();
                if let Err(error) = history::append(
                    result.clone(),
                    "cascade",
                    settings.language.api_value().unwrap_or("auto"),
                    duration_ms,
                    settings.history_retention_days,
                ) {
                    log::error!("Failed to save transcription history: {}", error);
                } else {
                    let _ = app_clone.emit("history-changed", ());
                }

                let overlay = OverlayManager::new(app_clone.clone());
                let delivery_error = deliver_transcript(
                    &settings.automation,
                    transcript_delivery,
                    &result,
                    paste_target_pid,
                )
                .err();

                // A newer recording owns the overlay. The completed result is
                // still pasted and stored, but must not replace the red
                // recording indicator or hide it under the user's new speech.
                if !*app_clone.state::<AppState>().is_recording.lock() {
                    if let Some(error) = delivery_error.as_deref() {
                        overlay.show_paste_error(error);
                    } else {
                        overlay.show_preview(&result);
                    }
                    overlay.position_near_cursor(settings.overlay_centered);
                    let hide_delay = settings.hide_delay_clamped();
                    if hide_delay > 0.0 {
                        std::thread::sleep(std::time::Duration::from_secs_f64(hide_delay));
                        if !*app_clone.state::<AppState>().is_recording.lock() {
                            overlay.hide();
                        }
                    }
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
                *st.last_failed_delivery.lock() = transcript_delivery;

                let overlay = OverlayManager::new(app_clone.clone());
                if !*app_clone.state::<AppState>().is_recording.lock() {
                    overlay.show_retry(&e);
                    overlay.position_near_cursor(settings.overlay_centered);
                }

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

fn deliver_transcript(
    automation: &AutomationConfig,
    delivery: TranscriptDelivery,
    transcript: &str,
    paste_target_pid: Option<i32>,
) -> Result<(), String> {
    match automation::payload_for_transcript(automation, delivery, transcript) {
        AutomationPayload::Text(payload) => {
            log::info!("Running configured post-transcription automation");
            automation::run(automation, &payload)
        }
        AutomationPayload::EmptyAfterKeyword => {
            log::info!("Automation trigger had no payload; not sending or pasting it");
            Ok(())
        }
        AutomationPayload::NoMatch => {
            if transcript.is_empty() {
                return Ok(());
            }
            PasteboardTyper::new()
                .paste_to_pid(transcript, paste_target_pid)
                .map(|()| log::info!("Paste completed for target PID {:?}", paste_target_pid))
        }
    }
}

/// Retry transcription of the last failed audio.
#[tauri::command]
fn retry_transcription(app: tauri::AppHandle<Wry>, engine: Option<String>) -> Result<(), String> {
    let state = app.state::<AppState>();
    let audio_path = state
        .last_failed_audio
        .lock()
        .clone()
        .ok_or("No failed audio to retry")?;

    let settings = AppSettings::global().get();
    let paste_target_pid = *state.paste_target_pid.lock();
    let transcript_delivery = *state.last_failed_delivery.lock();
    let overlay = OverlayManager::new(app.clone());
    let selected_engine = retry_engine_from_ui(engine.as_deref())?;
    let retry_session = state.retry_session.fetch_add(1, Ordering::SeqCst) + 1;

    {
        let mut queue = state.queue.lock();
        queue.on_retry_started();
    }

    overlay.show_waiting();

    let app_clone = app.clone();
    std::thread::spawn(move || {
        let lang_code = settings.language.api_value();
        let result = match selected_engine {
            Some(engine) => match build_engine(&settings, engine) {
                Some(service) => {
                    log::info!("Retrying transcription with selected {:?} engine", engine);
                    service.transcribe(&audio_path, lang_code)
                }
                None => Err(format!(
                    "{} transcription is unavailable on this Mac. Configure it, then try again.",
                    engine.short()
                )),
            },
            None => make_cascade_transcriber(&settings).transcribe(&audio_path, lang_code),
        };

        match result {
            Ok(raw_text) => {
                if retry_session
                    != app_clone
                        .state::<AppState>()
                        .retry_session
                        .load(Ordering::SeqCst)
                {
                    log::info!("Retry result dismissed before completion; not pasting it");
                    app_clone
                        .state::<AppState>()
                        .queue
                        .lock()
                        .clear_busy_after_retry();
                    return;
                }
                let cleaned = text_cleaner::TextCleaner::clean(&raw_text);
                log::info!("Retry result: {}", cleaned);
                let duration_ms = audio_recorder::wav_duration_seconds(&audio_path)
                    .ok()
                    .map(|seconds| (seconds * 1000.0) as u64)
                    .unwrap_or_default();

                let _ = std::fs::remove_file(&audio_path);
                let st = app_clone.state::<AppState>();
                st.last_failed_audio.lock().take();

                let overlay = OverlayManager::new(app_clone.clone());
                let paste_error = deliver_transcript(
                    &settings.automation,
                    transcript_delivery,
                    &cleaned,
                    paste_target_pid,
                )
                .err();
                if !cleaned.is_empty() {
                    if let Err(error) = history::append(
                        cleaned.clone(),
                        "retry",
                        settings.language.api_value().unwrap_or("auto"),
                        duration_ms,
                        settings.history_retention_days,
                    ) {
                        log::error!("Failed to save retried transcription history: {}", error);
                    } else {
                        let _ = app_clone.emit("history-changed", ());
                    }
                }

                if !*app_clone.state::<AppState>().is_recording.lock() {
                    if let Some(error) = paste_error.as_deref() {
                        overlay.show_paste_error(error);
                    } else {
                        overlay.show_preview(&cleaned);
                    }
                    let hide_delay = settings.hide_delay_clamped();
                    if hide_delay > 0.0 {
                        std::thread::sleep(std::time::Duration::from_secs_f64(hide_delay));
                        if !*app_clone.state::<AppState>().is_recording.lock() {
                            overlay.hide();
                        }
                    }
                }

                let mut queue = st.queue.lock();
                let action = queue.on_transcription_completed();
                if action == RecordingAction::StartRecording {
                    drop(queue);
                    let _ = start_recording(app_clone);
                }
            }
            Err(e) => {
                if retry_session
                    != app_clone
                        .state::<AppState>()
                        .retry_session
                        .load(Ordering::SeqCst)
                {
                    log::info!("Retry error dismissed before completion");
                    app_clone
                        .state::<AppState>()
                        .queue
                        .lock()
                        .clear_busy_after_retry();
                    return;
                }
                log::error!("Retry error: {}", e);
                let overlay = OverlayManager::new(app_clone.clone());
                if !*app_clone.state::<AppState>().is_recording.lock() {
                    overlay.show_retry(&e);
                }
                let st = app_clone.state::<AppState>();
                let mut queue = st.queue.lock();
                queue.clear_busy_after_retry();
            }
        }
    });

    Ok(())
}

/// Close a failed transcription panel. This intentionally removes only the
/// temporary retry WAV; completed transcripts remain in their normal history.
#[tauri::command]
fn dismiss_transcription_error(app: tauri::AppHandle<Wry>) -> Result<(), String> {
    let state = app.state::<AppState>();
    state.retry_session.fetch_add(1, Ordering::SeqCst);
    if let Some(path) = state.last_failed_audio.lock().take() {
        if let Err(error) = std::fs::remove_file(&path) {
            if error.kind() != std::io::ErrorKind::NotFound {
                log::warn!(
                    "Failed to remove dismissed retry audio {}: {}",
                    path.display(),
                    error
                );
            }
        }
    }
    OverlayManager::new(app).hide();
    Ok(())
}

/// Hide a transient overlay message that has no retry audio to discard, such
/// as a paste or hotkey permission error.
#[tauri::command]
fn dismiss_overlay(app: tauri::AppHandle<Wry>) -> Result<(), String> {
    OverlayManager::new(app).hide();
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
            let menu = tray_manager
                .build_menu()
                .map_err(|e| tauri::Error::Anyhow(anyhow::anyhow!(e)))?;

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

            // Populate the tray's remote model picker without blocking app startup.
            let model_handle = handle.clone();
            std::thread::spawn(move || {
                let models = settings_commands::refresh_remote_models_cache();
                if !models.is_empty() {
                    TrayManager::new(model_handle).rebuild();
                }
            });

            // Register global hotkey
            let settings = AppSettings::global().get();
            let state = app.state::<AppState>();
            let mut hotkey = state.hotkey.lock();
            if let Err(error) = hotkey.register(&handle, settings.hotkey) {
                log::error!("Hotkey registration failed: {}", error);
                // The overlay used to receive this event while still hidden,
                // and its CSS hid the error text as well. Show a real visible
                // error state so a failed hotkey can never look like a silent
                // no-op.
                let overlay = OverlayManager::new(handle.clone());
                overlay.show_paste_error(&error);
                overlay.position_near_cursor(settings.overlay_centered);
                let _ = handle.emit("hotkey-error", error);
            }
            if let Err(error) = hotkey
                .register_automation(&handle, settings.automation.requires_fn_control_monitor())
            {
                log::error!("Automation hotkey registration failed: {}", error);
                let overlay = OverlayManager::new(handle.clone());
                overlay.show_paste_error(&error);
                overlay.position_near_cursor(settings.overlay_centered);
            }

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

            // Fn + Control is reserved for the configured automation. It
            // always uses hold-to-record semantics, independent of the normal
            // dictation hotkey's Hold/Toggle setting.
            let automation_hotkey_handle = handle.clone();
            app.listen("automation-hotkey-pressed", move |_event| {
                let state = automation_hotkey_handle.state::<AppState>();
                if *state.is_recording.lock() {
                    // If Fn reached the normal listener a few milliseconds
                    // before Control, preserve the already-open recorder and
                    // re-route this very recording to automation. This keeps
                    // the chord reliable regardless of key press order.
                    *state.transcript_delivery.lock() = TranscriptDelivery::AutomationHotkey;
                    log::info!("Re-routed active Fn recording to automation");
                } else {
                    let _ = start_recording_with_delivery(
                        automation_hotkey_handle.clone(),
                        TranscriptDelivery::AutomationHotkey,
                    );
                }
            });

            let automation_hotkey_release_handle = handle.clone();
            app.listen("automation-hotkey-released", move |_event| {
                let state = automation_hotkey_release_handle.state::<AppState>();
                let delivery = *state.transcript_delivery.lock();
                if *state.is_recording.lock() && delivery == TranscriptDelivery::AutomationHotkey {
                    let _ = stop_and_transcribe(automation_hotkey_release_handle.clone());
                }
            });

            // Do this on every launch while a required permission is missing.
            // The Settings window opens directly on its Permissions section so
            // users do not have to guess why the hotkey or paste path is dead.
            let permissions = settings_commands::check_permissions();
            if settings_commands::permissions_missing(&settings, &permissions) {
                if let Some(window) = handle.get_webview_window("settings") {
                    let _ = window.show();
                    let _ = window.set_focus();
                }
                let _ = handle.emit("permission-setup-required", ());
                log::warn!(
                    "Required permissions are missing at startup: {}",
                    permissions
                );
            }

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
            dismiss_transcription_error,
            dismiss_overlay,
            save_endpoint,
            save_api_key,
            get_settings,
            save_settings,
            refresh_remote_models,
            open_settings,
            download_local_model,
            get_history,
            clear_history,
            open_models_folder,
            open_model_page,
            open_permissions,
            request_permissions,
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
    fn retry_choice_parses_only_the_visible_methods() {
        assert_eq!(retry_engine_from_ui(None), Ok(None));
        assert_eq!(
            retry_engine_from_ui(Some("remote")),
            Ok(Some(SttEngine::Remote))
        );
        assert_eq!(
            retry_engine_from_ui(Some("native")),
            Ok(Some(SttEngine::Native))
        );
        assert_eq!(
            retry_engine_from_ui(Some("local")),
            Ok(Some(SttEngine::Local))
        );
        assert!(retry_engine_from_ui(Some("cascade")).is_err());
    }

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
