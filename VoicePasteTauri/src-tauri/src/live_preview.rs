//! Periodic partial transcription while a hold-to-record session is active.
//!
//! Each pass uses a finalized WAV snapshot, so the microphone callback never
//! shares an incomplete file with the transcriber.

use crate::config::AppConfig;
use crate::overlay::OverlayManager;
use crate::{make_cascade_transcriber, AppState};
use std::path::PathBuf;
use std::sync::atomic::Ordering;
use tauri::{AppHandle, Manager, Wry};

pub fn start(app: AppHandle<Wry>, settings: AppConfig, recording_path: PathBuf, session: u64) {
    if !settings.realtime_preview {
        return;
    }

    std::thread::spawn(move || {
        let interval =
            std::time::Duration::from_secs_f64(settings.realtime_chunk_interval_clamped());
        loop {
            std::thread::sleep(interval);

            let snapshot = {
                let state = app.state::<AppState>();
                if !is_current_recording(&state, &recording_path, session) {
                    break;
                }
                let recorder = state.recorder.lock();
                if recorder.current_path() != Some(&recording_path) {
                    break;
                }
                recorder.preview_snapshot()
            };

            let snapshot = match snapshot {
                Some(snapshot) => match snapshot.write_to_temp_file() {
                    Ok(path) => path,
                    Err(error) => {
                        log::warn!("Could not write live-preview audio: {}", error);
                        continue;
                    }
                },
                None => continue,
            };

            let state = app.state::<AppState>();
            if state
                .preview_in_flight
                .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
                .is_err()
            {
                let _ = std::fs::remove_file(&snapshot);
                continue;
            }
            drop(state);

            let result = make_cascade_transcriber(&settings)
                .transcribe(&snapshot, settings.language.api_value())
                .map(|raw| crate::text_cleaner::TextCleaner::clean(&raw));
            let _ = std::fs::remove_file(&snapshot);
            app.state::<AppState>()
                .preview_in_flight
                .store(false, Ordering::SeqCst);

            let Ok(text) = result else {
                continue;
            };
            if text.is_empty() {
                continue;
            }

            let state = app.state::<AppState>();
            if !is_current_recording(&state, &recording_path, session) {
                break;
            }

            *state.preview_text.lock() = text.clone();
            OverlayManager::new(app.clone()).show_preview(&text);
        }
    });
}

fn is_current_recording(state: &AppState, recording_path: &PathBuf, session: u64) -> bool {
    if state.preview_session.load(Ordering::SeqCst) != session || !*state.is_recording.lock() {
        return false;
    }
    let recorder = state.recorder.lock();
    recorder.current_path() == Some(recording_path)
}
