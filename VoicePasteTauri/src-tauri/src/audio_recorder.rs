use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{SampleFormat, Stream};
use hound::{WavSpec, WavWriter};
use parking_lot::Mutex;
use std::collections::VecDeque;
use std::io::BufWriter;
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// Minimum recording length we consider worth transcribing.
///
/// If the user releases the hotkey faster than this (in seconds), the
/// resulting WAV has so few samples that the audio buffer is effectively
/// silence — even a real human utterance can't be inferred from it. The
/// transcription pipeline in `lib.rs::stop_and_transcribe` short-circuits
/// with `Ok("")` when the WAV is shorter than this threshold, avoiding a
/// wasted server round-trip and a `whisper.cpp` "empty input" error.
///
/// Set slightly below the default `recording_delay` (0.20s) so the
/// user has visual feedback that the recording started, but anything
/// shorter than a syllable is rejected outright.
pub const MIN_RECORDING_DURATION_S: f64 = 0.15;
const MAX_PREVIEW_SECONDS: usize = 120;

/// Read the duration of a WAV file in seconds, computed from its header
/// (no PCM decoding). Returns 0.0 for an empty WAV (header only, no
/// frames) or if the sample rate is missing/zero in the header.
///
/// Used by `lib.rs::stop_and_transcribe` to detect "recording too short"
/// without paying the cost of a full sample decode.
pub fn wav_duration_seconds(path: &Path) -> Result<f64, String> {
    let reader =
        hound::WavReader::open(path).map_err(|e| format!("Cannot read WAV header: {}", e))?;
    let spec = reader.spec();
    if spec.sample_rate == 0 {
        return Ok(0.0);
    }
    Ok(reader.duration() as f64 / spec.sample_rate as f64)
}

/// Wrapper around cpal::Stream to make it Send + Sync.
/// Safety: cpal::Stream is !Send on some platforms, but we only access it
/// while holding the AudioRecorder mutex, so no concurrent access occurs.
#[allow(dead_code)]
struct SendStream(Stream);
unsafe impl Send for SendStream {}
unsafe impl Sync for SendStream {}

struct PreviewBuffer {
    samples: VecDeque<i16>,
    max_samples: usize,
    first_sample: u64,
    next_sample: u64,
}

impl PreviewBuffer {
    fn new(max_samples: usize) -> Self {
        Self {
            samples: VecDeque::with_capacity(max_samples),
            max_samples,
            first_sample: 0,
            next_sample: 0,
        }
    }

    fn push_samples(&mut self, samples: impl IntoIterator<Item = i16>) {
        for sample in samples {
            if self.samples.len() == self.max_samples {
                self.samples.pop_front();
                self.first_sample = self.first_sample.saturating_add(1);
            }
            self.samples.push_back(sample);
            self.next_sample = self.next_sample.saturating_add(1);
        }
    }

    fn snapshot_since(&self, cursor: u64, spec: WavSpec) -> Option<PreviewSnapshot> {
        let start_sample = cursor.max(self.first_sample).min(self.next_sample);
        let offset = start_sample.saturating_sub(self.first_sample) as usize;
        let samples = self
            .samples
            .iter()
            .skip(offset)
            .copied()
            .collect::<Vec<_>>();
        if samples.is_empty() {
            return None;
        }
        Some(PreviewSnapshot {
            spec,
            samples,
            start_sample,
            end_sample: self.next_sample,
        })
    }

    fn clear(&mut self) {
        self.samples.clear();
        self.first_sample = 0;
        self.next_sample = 0;
    }
}

/// A bounded, finalized-in-memory audio snapshot. Writing it to disk happens
/// after the recorder lock is released, so releasing Fn is never blocked on
/// preview file I/O.
pub struct PreviewSnapshot {
    pub(crate) spec: WavSpec,
    pub(crate) samples: Vec<i16>,
    pub(crate) start_sample: u64,
    pub(crate) end_sample: u64,
}

impl PreviewSnapshot {
    pub(crate) fn from_samples(
        spec: WavSpec,
        samples: Vec<i16>,
        start_sample: u64,
        end_sample: u64,
    ) -> Self {
        Self {
            spec,
            samples,
            start_sample,
            end_sample,
        }
    }

    pub fn write_to_temp_file(self) -> Result<PathBuf, String> {
        let path = std::env::temp_dir()
            .join("voicepaste-recordings")
            .join(format!("preview-{}.wav", uuid_simple()));
        let mut writer = WavWriter::create(&path, self.spec)
            .map_err(|error| format!("Cannot create preview WAV: {}", error))?;
        for sample in self.samples {
            writer
                .write_sample(sample)
                .map_err(|error| format!("Cannot write preview WAV: {}", error))?;
        }
        writer
            .finalize()
            .map_err(|error| format!("Cannot finalize preview WAV: {}", error))?;
        Ok(path)
    }
}

/// Records audio from the default microphone to a WAV file.
pub struct AudioRecorder {
    stream: Option<SendStream>,
    current_path: Option<PathBuf>,
    writer: Arc<Mutex<Option<WavWriter<BufWriter<std::fs::File>>>>>,
    preview_samples: Arc<Mutex<PreviewBuffer>>,
    preview_spec: Option<WavSpec>,
}

impl AudioRecorder {
    pub fn new() -> Self {
        Self {
            stream: None,
            current_path: None,
            writer: Arc::new(Mutex::new(None)),
            preview_samples: Arc::new(Mutex::new(PreviewBuffer::new(1))),
            preview_spec: None,
        }
    }

    /// Current recording file path (set after start, cleared after stop).
    pub fn current_path(&self) -> Option<&PathBuf> {
        self.current_path.as_ref()
    }

    pub fn preview_spec(&self) -> Option<WavSpec> {
        self.preview_spec
    }

    /// Start recording from the default input device.
    pub fn start(&mut self) -> Result<(), String> {
        self.stop_internal(false);

        let host = cpal::default_host();
        let device = host
            .default_input_device()
            .ok_or("No input device available")?;

        let config = device
            .default_input_config()
            .map_err(|e| format!("Failed to get input config: {}", e))?;

        let sample_rate = config.sample_rate().0;
        let channels = config.channels() as u16;

        // Create temp file
        let dir = std::env::temp_dir().join("voicepaste-recordings");
        std::fs::create_dir_all(&dir).map_err(|e| format!("Cannot create dir: {}", e))?;
        let path = dir.join(format!("{}.wav", uuid_simple()));

        // Set up WAV writer writing directly to file
        let spec = WavSpec {
            channels,
            sample_rate,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };

        let file =
            std::fs::File::create(&path).map_err(|e| format!("Cannot create file: {}", e))?;
        let buf_writer = BufWriter::new(file);
        let wav_writer = WavWriter::new(buf_writer, spec)
            .map_err(|e| format!("Cannot create WAV writer: {}", e))?;

        let writer = Arc::new(Mutex::new(Some(wav_writer)));
        let writer_clone = writer.clone();
        let preview_max_samples = sample_rate as usize * channels as usize * MAX_PREVIEW_SECONDS;
        let preview_samples = Arc::new(Mutex::new(PreviewBuffer::new(preview_max_samples)));
        let preview_samples_clone = preview_samples.clone();
        // A deterministic live-preview fixture is used only by the macOS
        // black-box canary. Keep recording the real input so the normal WAV
        // lifecycle is exercised, but do not mix room noise into the fixture
        // that VAD consumes.
        let preview_uses_fixture =
            std::env::var("VOICEPASTE_TEST_LIVE_AUDIO").is_ok_and(|path| !path.trim().is_empty());

        let stream_config = cpal::StreamConfig {
            channels: channels.into(),
            sample_rate: config.sample_rate(),
            buffer_size: cpal::BufferSize::Default,
        };

        let stream = match config.sample_format() {
            SampleFormat::I16 => device.build_input_stream(
                &stream_config,
                move |data: &[i16], _: &cpal::InputCallbackInfo| {
                    let mut guard = writer_clone.lock();
                    if let Some(w) = guard.as_mut() {
                        for &sample in data {
                            let _ = w.write_sample(sample);
                        }
                    }
                    if !preview_uses_fixture {
                        preview_samples_clone
                            .lock()
                            .push_samples(data.iter().copied());
                    }
                },
                |err| {
                    log::error!("Audio stream error: {}", err);
                },
                None,
            ),
            SampleFormat::F32 => device.build_input_stream(
                &stream_config,
                move |data: &[f32], _: &cpal::InputCallbackInfo| {
                    let mut guard = writer_clone.lock();
                    if let Some(w) = guard.as_mut() {
                        for &sample in data {
                            let _ = w.write_sample((sample * i16::MAX as f32) as i16);
                        }
                    }
                    if !preview_uses_fixture {
                        preview_samples_clone.lock().push_samples(
                            data.iter().map(|sample| (sample * i16::MAX as f32) as i16),
                        );
                    }
                },
                |err| {
                    log::error!("Audio stream error: {}", err);
                },
                None,
            ),
            _ => return Err("Unsupported sample format".to_string()),
        };

        let stream = stream.map_err(|e| format!("Failed to build input stream: {}", e))?;
        stream
            .play()
            .map_err(|e| format!("Failed to start stream: {}", e))?;

        self.stream = Some(SendStream(stream));
        self.writer = writer;
        self.preview_samples = preview_samples;
        self.preview_spec = Some(spec);
        self.current_path = Some(path);
        if let Ok(path) = std::env::var("VOICEPASTE_TEST_LIVE_AUDIO") {
            if !path.trim().is_empty() {
                match read_test_preview_samples(Path::new(&path), spec) {
                    Ok(samples) => {
                        log::warn!(
                            "preloading {} samples from VOICEPASTE_TEST_LIVE_AUDIO",
                            samples.len()
                        );
                        self.preview_samples.lock().push_samples(samples);
                    }
                    Err(error) => log::error!("could not preload live-preview fixture: {}", error),
                }
            }
        }
        Ok(())
    }

    /// Copy only samples recorded after `cursor` for background VAD.
    ///
    /// Absolute sample indices make overlap impossible: the caller advances
    /// its cursor to `end_sample`, and a later snapshot begins there. If a
    /// stalled preview falls behind the bounded buffer, the returned start is
    /// moved forward to the oldest retained sample instead of replaying audio.
    pub fn preview_snapshot_since(&self, cursor: u64) -> Option<PreviewSnapshot> {
        let Some(spec) = self.preview_spec else {
            return None;
        };
        self.preview_samples.lock().snapshot_since(cursor, spec)
    }

    /// Stop recording and return the path to the WAV file.
    pub fn stop(&mut self) -> Option<PathBuf> {
        self.stop_internal(true)
    }

    /// Stop recording without returning the file (discards it).
    pub fn stop_and_discard(&mut self) {
        self.stop_internal(false);
    }

    fn stop_internal(&mut self, keep_file: bool) -> Option<PathBuf> {
        // Drop the stream first to stop capturing
        self.stream = None;

        // Finalize the WAV writer
        let path = self.current_path.take();
        self.preview_spec = None;
        self.preview_samples.lock().clear();
        let writer_guard = self.writer.lock().take();

        if let (Some(path), Some(writer)) = (path, writer_guard) {
            if keep_file {
                // Finalize the WAV file (writes header with correct sizes)
                if writer.finalize().is_ok() {
                    return Some(path);
                }
            }
            // Discard
            let _ = std::fs::remove_file(&path);
        }

        None
    }
}

fn read_test_preview_samples(path: &Path, expected: WavSpec) -> Result<Vec<i16>, String> {
    let mut reader = hound::WavReader::open(path)
        .map_err(|error| format!("Cannot open live-preview fixture: {}", error))?;
    let spec = reader.spec();
    if spec.sample_format != hound::SampleFormat::Int || spec.bits_per_sample != 16 {
        return Err("live-preview fixture must be 16-bit PCM".to_string());
    }
    if spec.channels == 0 || spec.sample_rate == 0 || expected.channels == 0 {
        return Err("live-preview fixture has invalid audio metadata".to_string());
    }

    let interleaved = reader
        .samples::<i16>()
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("Cannot read live-preview fixture samples: {}", error))?;
    let source_channels = spec.channels as usize;
    let mono = interleaved
        .chunks_exact(source_channels)
        .map(|frame| {
            let sum = frame.iter().map(|sample| *sample as i32).sum::<i32>();
            (sum / source_channels as i32) as i16
        })
        .collect::<Vec<_>>();
    if mono.is_empty() {
        return Ok(Vec::new());
    }

    let target_frames = ((mono.len() as u64 * expected.sample_rate as u64)
        .div_ceil(spec.sample_rate as u64)) as usize;
    let mut converted = Vec::with_capacity(target_frames * expected.channels as usize);
    for output_frame in 0..target_frames {
        let source_frame = ((output_frame as u64 * spec.sample_rate as u64)
            / expected.sample_rate as u64) as usize;
        let sample = mono[source_frame.min(mono.len() - 1)];
        converted.extend(std::iter::repeat_n(sample, expected.channels as usize));
    }
    Ok(converted)
}

fn uuid_simple() -> String {
    use rand::Rng;
    let mut rng = rand::thread_rng();
    format!("{:016x}{:016x}", rng.gen::<u64>(), rng.gen::<u64>())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper: write a WAV at `path` with `n_samples` frames of 16-bit
    /// silence (all zeros) at the given sample rate. Used by the
    /// duration / short-recording tests below.
    fn write_silent_wav(path: &Path, sample_rate: u32, n_samples: u16) {
        let spec = WavSpec {
            channels: 1,
            sample_rate,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };
        let mut writer = hound::WavWriter::create(path, spec).expect("create WAV");
        for _ in 0..n_samples {
            writer.write_sample(0i16).expect("write sample");
        }
        writer.finalize().expect("finalize WAV");
    }

    #[test]
    fn wav_duration_seconds_empty_file_is_zero() {
        // Header-only WAV, 0 samples — the case cpal produces when the
        // hotkey is released before the first audio buffer arrives.
        let path = std::env::temp_dir().join("voicepaste_test_empty_dur.wav");
        let _ = std::fs::remove_file(&path);
        write_silent_wav(&path, 16000, 0);

        let d = wav_duration_seconds(&path).expect("duration should be Ok");
        assert_eq!(d, 0.0, "empty WAV should report 0.0s duration");
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn wav_duration_seconds_short_recording_below_threshold() {
        // 50ms of silence at 16kHz = 800 samples. Well below the
        // MIN_RECORDING_DURATION_S threshold (0.15s = 2400 samples).
        // This mirrors a real user tapping Fn and releasing quickly.
        let path = std::env::temp_dir().join("voicepaste_test_short_dur.wav");
        let _ = std::fs::remove_file(&path);
        write_silent_wav(&path, 16000, 800);

        let d = wav_duration_seconds(&path).expect("duration should be Ok");
        assert!(
            d < MIN_RECORDING_DURATION_S,
            "expected duration ({:.4}s) to be below MIN_RECORDING_DURATION_S ({:.2}s)",
            d,
            MIN_RECORDING_DURATION_S
        );
        // Sanity: the threshold itself is sane.
        assert!(MIN_RECORDING_DURATION_S > 0.0);
        assert!(MIN_RECORDING_DURATION_S < 1.0);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn wav_duration_seconds_longer_recording_above_threshold() {
        // 0.5s of silence at 16kHz = 8000 samples. Above threshold.
        let path = std::env::temp_dir().join("voicepaste_test_long_dur.wav");
        let _ = std::fs::remove_file(&path);
        write_silent_wav(&path, 16000, 8000);

        let d = wav_duration_seconds(&path).expect("duration should be Ok");
        assert!(
            d >= MIN_RECORDING_DURATION_S,
            "expected duration ({:.4}s) to be at or above MIN_RECORDING_DURATION_S ({:.2}s)",
            d,
            MIN_RECORDING_DURATION_S
        );
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn wav_duration_seconds_handles_missing_file() {
        let path = std::env::temp_dir().join("voicepaste_test_does_not_exist_xyz.wav");
        let _ = std::fs::remove_file(&path);
        let result = wav_duration_seconds(&path);
        assert!(result.is_err(), "missing file should return Err");
    }

    #[test]
    fn preview_snapshots_contain_only_samples_after_the_cursor() {
        let spec = WavSpec {
            channels: 1,
            sample_rate: 16_000,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };
        let mut buffer = PreviewBuffer::new(16);
        buffer.push_samples([10, 11, 12]);
        let first = buffer.snapshot_since(0, spec).expect("first delta");
        assert_eq!((first.start_sample, first.end_sample), (0, 3));
        assert_eq!(first.samples, vec![10, 11, 12]);

        buffer.push_samples([20, 21]);
        let second = buffer
            .snapshot_since(first.end_sample, spec)
            .expect("second delta");
        assert_eq!((second.start_sample, second.end_sample), (3, 5));
        assert_eq!(second.samples, vec![20, 21]);
    }

    #[test]
    fn lagging_preview_skips_dropped_audio_instead_of_replaying_it() {
        let spec = WavSpec {
            channels: 1,
            sample_rate: 16_000,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };
        let mut buffer = PreviewBuffer::new(3);
        buffer.push_samples([1, 2, 3, 4, 5]);
        let delta = buffer.snapshot_since(0, spec).expect("retained delta");
        assert_eq!((delta.start_sample, delta.end_sample), (2, 5));
        assert_eq!(delta.samples, vec![3, 4, 5]);
    }

    #[test]
    fn live_fixture_is_resampled_and_duplicated_for_microphone_format() {
        let path = std::env::temp_dir().join("voicepaste_test_live_fixture_convert.wav");
        let _ = std::fs::remove_file(&path);
        let source = WavSpec {
            channels: 1,
            sample_rate: 16_000,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };
        let mut writer = hound::WavWriter::create(&path, source).expect("create fixture");
        for sample in [3_000i16, 6_000] {
            writer.write_sample(sample).expect("write fixture sample");
        }
        writer.finalize().expect("finalize fixture");

        let target = WavSpec {
            channels: 2,
            sample_rate: 48_000,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };
        let converted = read_test_preview_samples(&path, target).expect("convert fixture");
        assert_eq!(converted.len(), 12);
        assert_eq!(&converted[..6], &[3_000; 6]);
        assert_eq!(&converted[6..], &[6_000; 6]);
        let _ = std::fs::remove_file(&path);
    }
}
