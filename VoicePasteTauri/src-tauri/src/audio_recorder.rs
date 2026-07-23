use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{SampleFormat, Stream};
use hound::{WavSpec, WavWriter};
use parking_lot::Mutex;
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

/// Records audio from the default microphone to a WAV file.
pub struct AudioRecorder {
    stream: Option<SendStream>,
    current_path: Option<PathBuf>,
    writer: Arc<Mutex<Option<WavWriter<BufWriter<std::fs::File>>>>>,
}

impl AudioRecorder {
    pub fn new() -> Self {
        Self {
            stream: None,
            current_path: None,
            writer: Arc::new(Mutex::new(None)),
        }
    }

    /// Current recording file path (set after start, cleared after stop).
    pub fn current_path(&self) -> Option<&PathBuf> {
        self.current_path.as_ref()
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
        self.current_path = Some(path);
        Ok(())
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
}
