use crate::audio_recorder::AudioRecorder;
use crate::automation::TranscriptDelivery;
use crate::hotkey::HotkeyManager;
use crate::recording_queue::RecordingQueueCoordinator;
use crate::wake_wav::WakeWav;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64};

/// Shared mutable runtime state owned by the Tauri application.
pub struct AppState {
    pub recorder: parking_lot::Mutex<AudioRecorder>,
    pub queue: parking_lot::Mutex<RecordingQueueCoordinator>,
    pub hotkey: parking_lot::Mutex<HotkeyManager>,
    pub wake_wav: parking_lot::Mutex<WakeWav>,
    pub preview_text: parking_lot::Mutex<String>,
    pub(crate) preview_runtime: parking_lot::Mutex<Option<crate::live_preview::LivePreviewRuntime>>,
    pub last_failed_audio: parking_lot::Mutex<Option<PathBuf>>,
    pub last_failed_delivery: parking_lot::Mutex<TranscriptDelivery>,
    /// Invalidation token for a retry the user has explicitly closed or
    /// replaced with another engine. Recording transcriptions deliberately
    /// remain independent; this only controls the user-driven retry surface.
    pub retry_session: AtomicU64,
    pub is_recording: parking_lot::Mutex<bool>,
    pub paste_target_pid: parking_lot::Mutex<Option<i32>>,
    pub transcript_delivery: parking_lot::Mutex<TranscriptDelivery>,
    pub preview_session: AtomicU64,
    pub preview_in_flight: AtomicBool,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            recorder: parking_lot::Mutex::new(AudioRecorder::new()),
            queue: parking_lot::Mutex::new(RecordingQueueCoordinator::new()),
            hotkey: parking_lot::Mutex::new(HotkeyManager::new()),
            wake_wav: parking_lot::Mutex::new(WakeWav::new()),
            preview_text: parking_lot::Mutex::new(String::new()),
            preview_runtime: parking_lot::Mutex::new(None),
            last_failed_audio: parking_lot::Mutex::new(None),
            last_failed_delivery: parking_lot::Mutex::new(TranscriptDelivery::Paste),
            retry_session: AtomicU64::new(0),
            is_recording: parking_lot::Mutex::new(false),
            paste_target_pid: parking_lot::Mutex::new(None),
            transcript_delivery: parking_lot::Mutex::new(TranscriptDelivery::Paste),
            preview_session: AtomicU64::new(0),
            preview_in_flight: AtomicBool::new(false),
        }
    }
}
