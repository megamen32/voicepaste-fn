//! Incremental VAD-driven transcription while a recording is active.
//!
//! The recorder exposes absolute sample cursors. Every sample is therefore
//! inspected at most once, and only completed speech chunks are sent to STT.
//! Chunk text updates the preview and clipboard, but never injects Cmd/Ctrl+V;
//! insertion remains the responsibility of the later full-file pass.

use crate::audio_recorder::PreviewSnapshot;
use crate::config::AppConfig;
use crate::overlay::OverlayManager;
use crate::pasteboard_typer::PasteboardTyper;
use crate::{make_cascade_transcriber, AppState};
use hound::WavSpec;
use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::atomic::Ordering;
use tauri::{AppHandle, Manager, Wry};

const VAD_POLL_MS: u64 = 80;
const VAD_FRAME_MS: usize = 20;
const VAD_PRE_ROLL_MS: usize = 140;
const VAD_TRAILING_AUDIO_MS: usize = 120;
const VAD_MIN_SPEECH_MS: usize = 240;
const VAD_MAX_CHUNK_MS: usize = 20_000;

pub(crate) struct LivePreviewRuntime {
    session: u64,
    cursor: u64,
    vad: VadChunker,
}

impl LivePreviewRuntime {
    pub(crate) fn new(session: u64, spec: WavSpec, settings: &AppConfig) -> Self {
        Self {
            session,
            cursor: 0,
            vad: VadChunker::new(
                spec,
                settings.vad_sensitivity_clamped(),
                settings.vad_silence_ms_clamped(),
            ),
        }
    }

    pub(crate) fn session(&self) -> u64 {
        self.session
    }

    pub(crate) fn cursor(&self) -> u64 {
        self.cursor
    }

    pub(crate) fn push(&mut self, snapshot: PreviewSnapshot) -> Vec<PreviewSnapshot> {
        self.cursor = snapshot.end_sample;
        self.vad.push(snapshot)
    }

    pub(crate) fn finish(&mut self) -> Vec<PreviewSnapshot> {
        self.vad.finish().into_iter().collect()
    }
}

pub fn start(app: AppHandle<Wry>, settings: AppConfig, recording_path: PathBuf, session: u64) {
    if !settings.realtime_preview {
        return;
    }

    std::thread::spawn(move || loop {
        std::thread::sleep(std::time::Duration::from_millis(VAD_POLL_MS));

        let chunks = take_ready_chunks(&app, &recording_path, session);
        transcribe_batch(&app, &settings, session, chunks);

        let state = app.state::<AppState>();
        if !is_current_recording(&state, &recording_path, session) {
            break;
        }
    });
}

/// Finish VAD chunks captured immediately before the recorder was stopped.
///
/// A chunk request that was already in flight is allowed to publish its draft
/// first. This prevents an older preview from overwriting the clipboard after
/// the final full-file result has been pasted.
pub fn finish_pending(
    app: &AppHandle<Wry>,
    settings: &AppConfig,
    session: u64,
    chunks: Vec<PreviewSnapshot>,
) {
    while app
        .state::<AppState>()
        .preview_in_flight
        .load(Ordering::SeqCst)
    {
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    if !chunks.is_empty() {
        app.state::<AppState>()
            .preview_in_flight
            .store(true, Ordering::SeqCst);
    }
    transcribe_batch(app, settings, session, chunks);
}

fn take_ready_chunks(
    app: &AppHandle<Wry>,
    recording_path: &PathBuf,
    session: u64,
) -> Vec<PreviewSnapshot> {
    let state = app.state::<AppState>();
    if !is_current_recording(&state, recording_path, session) {
        return Vec::new();
    }

    // Runtime -> recorder is the single lock order used here and by the stop
    // path, so a simultaneous key release cannot duplicate or lose a delta.
    let mut runtime_guard = state.preview_runtime.lock();
    let Some(runtime) = runtime_guard.as_mut() else {
        return Vec::new();
    };
    if runtime.session() != session {
        return Vec::new();
    }
    let recorder = state.recorder.lock();
    if recorder.current_path() != Some(recording_path) {
        return Vec::new();
    }
    let chunks = recorder
        .preview_snapshot_since(runtime.cursor())
        .map(|snapshot| runtime.push(snapshot))
        .unwrap_or_default();
    // Mark the entire returned batch in flight before releasing the runtime
    // and recorder locks. The stop path therefore cannot observe a false gap
    // between two phrases produced by the same VAD poll.
    if !chunks.is_empty() {
        state.preview_in_flight.store(true, Ordering::SeqCst);
    }
    chunks
}

fn transcribe_batch(
    app: &AppHandle<Wry>,
    settings: &AppConfig,
    session: u64,
    chunks: Vec<PreviewSnapshot>,
) {
    for chunk in chunks {
        transcribe_chunk(app, settings, session, chunk);
    }
    app.state::<AppState>()
        .preview_in_flight
        .store(false, Ordering::SeqCst);
}

fn transcribe_chunk(
    app: &AppHandle<Wry>,
    settings: &AppConfig,
    session: u64,
    chunk: PreviewSnapshot,
) {
    let range = (chunk.start_sample, chunk.end_sample);
    let path = match chunk.write_to_temp_file() {
        Ok(path) => path,
        Err(error) => {
            log::warn!("Could not write VAD preview chunk: {}", error);
            return;
        }
    };
    log::info!("Transcribing new VAD sample range {}..{}", range.0, range.1);
    let result = make_cascade_transcriber(settings)
        .transcribe(&path, settings.language.api_value())
        .map(|raw| crate::text_cleaner::TextCleaner::clean(&raw));
    let _ = std::fs::remove_file(&path);

    let Ok(text) = result else {
        log::warn!(
            "VAD preview transcription failed for range {}..{}",
            range.0,
            range.1
        );
        return;
    };
    if text.is_empty()
        || app
            .state::<AppState>()
            .preview_session
            .load(Ordering::SeqCst)
            != session
    {
        return;
    }

    let draft = {
        let state = app.state::<AppState>();
        let mut preview = state.preview_text.lock();
        append_chunk(&mut preview, &text);
        preview.clone()
    };
    if let Err(error) = PasteboardTyper::new().copy(&draft) {
        log::warn!("Could not copy live draft to clipboard: {}", error);
    } else {
        log::info!("Copied live draft to clipboard without inserting it");
    }
    OverlayManager::new(app.clone()).show_preview(&draft);
}

fn append_chunk(draft: &mut String, chunk: &str) {
    let chunk = chunk.trim();
    if chunk.is_empty() {
        return;
    }
    if !draft.is_empty() {
        draft.push(' ');
    }
    draft.push_str(chunk);
}

fn is_current_recording(state: &AppState, recording_path: &PathBuf, session: u64) -> bool {
    if state.preview_session.load(Ordering::SeqCst) != session || !*state.is_recording.lock() {
        return false;
    }
    let recorder = state.recorder.lock();
    recorder.current_path() == Some(recording_path)
}

struct VadChunker {
    spec: WavSpec,
    sensitivity: f64,
    silence_samples: usize,
    frame_samples: usize,
    pre_roll_samples: usize,
    trailing_audio_samples: usize,
    min_speech_samples: usize,
    max_chunk_samples: usize,
    noise_floor: f64,
    voiced_run: usize,
    speaking: bool,
    active_start: u64,
    active: Vec<i16>,
    trailing_silence: usize,
    voiced_samples: usize,
    pre_roll: VecDeque<i16>,
    pending: VecDeque<i16>,
    pending_start: u64,
    expected_next: Option<u64>,
}

impl VadChunker {
    fn new(spec: WavSpec, sensitivity: f64, silence_ms: u32) -> Self {
        let samples_per_ms = spec.sample_rate as usize * spec.channels as usize / 1000;
        Self {
            spec,
            sensitivity,
            silence_samples: samples_per_ms * silence_ms as usize,
            frame_samples: (samples_per_ms * VAD_FRAME_MS).max(spec.channels as usize),
            pre_roll_samples: samples_per_ms * VAD_PRE_ROLL_MS,
            trailing_audio_samples: samples_per_ms * VAD_TRAILING_AUDIO_MS,
            min_speech_samples: samples_per_ms * VAD_MIN_SPEECH_MS,
            max_chunk_samples: samples_per_ms * VAD_MAX_CHUNK_MS,
            noise_floor: 180.0,
            voiced_run: 0,
            speaking: false,
            active_start: 0,
            active: Vec::new(),
            trailing_silence: 0,
            voiced_samples: 0,
            pre_roll: VecDeque::with_capacity(samples_per_ms * VAD_PRE_ROLL_MS),
            pending: VecDeque::new(),
            pending_start: 0,
            expected_next: None,
        }
    }

    fn push(&mut self, snapshot: PreviewSnapshot) -> Vec<PreviewSnapshot> {
        if self
            .expected_next
            .is_some_and(|next| next != snapshot.start_sample)
        {
            log::warn!(
                "VAD cursor jumped from {:?} to {}; dropping incomplete phrase",
                self.expected_next,
                snapshot.start_sample
            );
            self.reset_phrase();
            self.pending.clear();
        }
        if self.pending.is_empty() {
            self.pending_start = snapshot.start_sample;
        }
        self.expected_next = Some(snapshot.end_sample);
        self.pending.extend(snapshot.samples);

        let mut chunks = Vec::new();
        while self.pending.len() >= self.frame_samples {
            let frame_start = self.pending_start;
            let frame = self.pending.drain(..self.frame_samples).collect::<Vec<_>>();
            self.pending_start += self.frame_samples as u64;
            if let Some(chunk) = self.process_frame(frame_start, &frame) {
                chunks.push(chunk);
            }
        }
        chunks
    }

    fn finish(&mut self) -> Option<PreviewSnapshot> {
        let mut completed = None;
        if !self.pending.is_empty() {
            let frame_start = self.pending_start;
            let frame = self.pending.drain(..).collect::<Vec<_>>();
            self.pending_start += frame.len() as u64;
            completed = self.process_frame(frame_start, &frame);
        }
        completed.or_else(|| self.emit_active_chunk())
    }

    fn process_frame(&mut self, frame_start: u64, frame: &[i16]) -> Option<PreviewSnapshot> {
        let rms = frame_rms(frame);
        let start_threshold = self.start_threshold();
        let end_threshold = (start_threshold * 0.62).max(self.noise_floor * 1.25);

        if !self.speaking {
            if rms < start_threshold {
                self.noise_floor = (self.noise_floor * 0.96 + rms * 0.04).clamp(35.0, 5_000.0);
                self.voiced_run = 0;
            } else {
                self.voiced_run += 1;
            }
            self.pre_roll.extend(frame.iter().copied());
            while self.pre_roll.len() > self.pre_roll_samples {
                self.pre_roll.pop_front();
            }
            if self.voiced_run < 2 {
                return None;
            }

            self.speaking = true;
            self.active_start = frame_start
                .saturating_add(frame.len() as u64)
                .saturating_sub(self.pre_roll.len() as u64);
            self.active.extend(self.pre_roll.drain(..));
            self.trailing_silence = 0;
            self.voiced_samples = frame.len() * self.voiced_run;
            return None;
        }

        self.active.extend_from_slice(frame);
        if rms >= end_threshold {
            self.trailing_silence = 0;
            self.voiced_samples += frame.len();
        } else {
            self.trailing_silence += frame.len();
        }

        if self.active.len() >= self.max_chunk_samples
            || self.trailing_silence >= self.silence_samples
        {
            return self.emit_active_chunk();
        }
        None
    }

    fn start_threshold(&self) -> f64 {
        let sensitivity = self.sensitivity.clamp(0.0, 1.0);
        let absolute_floor = 900.0 - sensitivity * 650.0;
        let noise_multiplier = 4.0 - sensitivity * 2.1;
        absolute_floor.max(self.noise_floor * noise_multiplier)
    }

    fn emit_active_chunk(&mut self) -> Option<PreviewSnapshot> {
        if !self.speaking {
            return None;
        }
        let trailing_to_remove = self
            .trailing_silence
            .saturating_sub(self.trailing_audio_samples)
            .min(self.active.len());
        self.active.truncate(self.active.len() - trailing_to_remove);
        let result = if self.voiced_samples >= self.min_speech_samples && !self.active.is_empty() {
            let samples = std::mem::take(&mut self.active);
            let end = self.active_start + samples.len() as u64;
            Some(PreviewSnapshot::from_samples(
                self.spec,
                samples,
                self.active_start,
                end,
            ))
        } else {
            self.active.clear();
            None
        };
        self.reset_phrase();
        result
    }

    fn reset_phrase(&mut self) {
        self.speaking = false;
        self.active.clear();
        self.trailing_silence = 0;
        self.voiced_samples = 0;
        self.voiced_run = 0;
        self.pre_roll.clear();
    }
}

fn frame_rms(frame: &[i16]) -> f64 {
    if frame.is_empty() {
        return 0.0;
    }
    let square_sum = frame
        .iter()
        .map(|sample| {
            let value = *sample as f64;
            value * value
        })
        .sum::<f64>();
    (square_sum / frame.len() as f64).sqrt()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spec() -> WavSpec {
        WavSpec {
            channels: 1,
            sample_rate: 1_000,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        }
    }

    fn snapshot(start: u64, samples: Vec<i16>) -> PreviewSnapshot {
        let end = start + samples.len() as u64;
        PreviewSnapshot::from_samples(spec(), samples, start, end)
    }

    #[test]
    fn vad_emits_non_overlapping_new_phrases() {
        let mut vad = VadChunker::new(spec(), 0.7, 300);
        let mut audio = Vec::new();
        audio.extend(vec![0; 200]);
        audio.extend(vec![3_000; 500]);
        audio.extend(vec![0; 400]);
        audio.extend(vec![6_000; 500]);
        audio.extend(vec![0; 400]);

        let chunks = vad.push(snapshot(0, audio));
        assert_eq!(chunks.len(), 2);
        assert!(chunks[0].end_sample <= chunks[1].start_sample);
        assert!(chunks[0].samples.iter().any(|sample| *sample == 3_000));
        assert!(!chunks[1].samples.iter().any(|sample| *sample == 3_000));
        assert!(chunks[1].samples.iter().any(|sample| *sample == 6_000));
    }

    #[test]
    fn vad_ignores_short_clicks_and_adapts_to_background_noise() {
        let mut vad = VadChunker::new(spec(), 0.6, 300);
        let mut audio = vec![120; 500];
        audio.extend(vec![2_000; 40]);
        audio.extend(vec![120; 500]);
        assert!(vad.push(snapshot(0, audio)).is_empty());
        assert!(vad.finish().is_none());
    }

    #[test]
    fn draft_assembler_preserves_chunk_order() {
        let mut draft = String::new();
        append_chunk(&mut draft, "первая фраза");
        append_chunk(&mut draft, "вторая фраза");
        assert_eq!(draft, "первая фраза вторая фраза");
    }
}
