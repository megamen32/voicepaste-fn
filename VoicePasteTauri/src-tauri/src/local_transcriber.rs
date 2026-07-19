use crate::transcription_service::TranscriptionService;
use std::path::Path;
use whisper_rs::{FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters};

/// Local on-device transcription using whisper.cpp via whisper-rs.
/// Cross-platform: works on macOS, Windows, and Linux.
pub struct LocalTranscriber {
    model_path: std::path::PathBuf,
}

impl LocalTranscriber {
    /// Create a new LocalTranscriber with the given whisper.cpp model path.
    pub fn new(model_path: std::path::PathBuf) -> Self {
        Self { model_path }
    }

    /// Try to find the whisper model file in standard locations.
    pub fn find_model() -> Option<std::path::PathBuf> {
        // Check env var first
        if let Ok(p) = std::env::var("WHISPER_MODEL_PATH") {
            let path = std::path::PathBuf::from(p);
            if path.exists() {
                return Some(path);
            }
        }

        // Check app data dirs
        let dirs = directories::ProjectDirs::from("com", "bezrabotnyi", "voicepaste");
        if let Some(dirs) = dirs {
            let path = dirs.data_dir().join("ggml-base.bin");
            if path.exists() {
                return Some(path);
            }
        }

        // Check current dir
        let local = std::path::PathBuf::from("ggml-base.bin");
        if local.exists() {
            return Some(local);
        }

        None
    }
}

impl TranscriptionService for LocalTranscriber {
    fn transcribe(&self, file_path: &Path, language_code: Option<&str>) -> Result<String, String> {
        if !self.model_path.exists() {
            return Err(format!(
                "Whisper model not found at: {}. Set WHISPER_MODEL_PATH env var.",
                self.model_path.display()
            ));
        }

        // Convert WAV to float samples for whisper
        let wav_data = std::fs::read(file_path)
            .map_err(|e| format!("Cannot read audio file: {}", e))?;

        let samples = wav_to_f32_samples(&wav_data)?;

        // Create whisper context
        let ctx = WhisperContext::new_with_params(
            self.model_path.to_str().unwrap_or(""),
            WhisperContextParameters::default(),
        )
        .map_err(|e| format!("Failed to create whisper context: {}", e))?;

        let mut state = ctx
            .create_state()
            .map_err(|e| format!("Failed to create whisper state: {}", e))?;

        let mut params = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });

        // Set language
        let lang_opt = language_code
            .filter(|l| *l != "auto");
        params.set_language(lang_opt);

        params.set_print_progress(false);
        params.set_print_timestamps(false);
        params.set_no_timestamps(true);
        params.set_single_segment(true);

        state
            .full(params, &samples)
            .map_err(|e| format!("Whisper transcription failed: {}", e))?;

        let num_segments = state
            .full_n_segments()
            .map_err(|e| format!("Failed to get segment count: {}", e))?;

        let mut result = String::new();
        for i in 0..num_segments {
            let segment = state
                .full_get_segment_text(i)
                .map_err(|e| format!("Failed to get segment text: {}", e))?;
            result.push_str(&segment);
        }

        let result = result.trim().to_string();
        if result.is_empty() {
            return Err("Local transcription returned empty result".to_string());
        }

        Ok(result)
    }
}

/// Convert WAV bytes (16-bit PCM) to f32 samples in [-1.0, 1.0].
/// Whisper expects 16kHz mono f32.
fn wav_to_f32_samples(wav_data: &[u8]) -> Result<Vec<f32>, String> {
    let cursor = std::io::Cursor::new(wav_data);
    let mut reader = hound::WavReader::new(cursor)
        .map_err(|e| format!("Invalid WAV file: {}", e))?;

    let spec = reader.spec();

    // We need mono 16kHz for whisper. If the file is different, we'll do basic conversion.
    let samples: Vec<f32> = match spec.sample_format {
        hound::SampleFormat::Int => {
            let max_val = (1u32 << (spec.bits_per_sample - 1)) as f32;
            reader
                .samples::<i16>()
                .map(|s| s.map(|v| v as f32 / max_val))
                .collect::<Result<Vec<_>, _>>()
                .map_err(|e| format!("WAV read error: {}", e))?
        }
        hound::SampleFormat::Float => reader
            .samples::<f32>()
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| format!("WAV read error: {}", e))?,
    };

    // If stereo, downmix to mono
    let mono = if spec.channels == 2 {
        samples
            .chunks(2)
            .map(|c| (c[0] + c[1]) / 2.0)
            .collect::<Vec<_>>()
    } else {
        samples
    };

    // If not 16kHz, do simple resampling (linear interpolation)
    if spec.sample_rate == 16000 {
        Ok(mono)
    } else {
        let ratio = 16000.0 / spec.sample_rate as f32;
        let new_len = (mono.len() as f32 * ratio) as usize;
        let mut resampled = Vec::with_capacity(new_len);
        for i in 0..new_len {
            let src_pos = i as f32 / ratio;
            let idx = src_pos as usize;
            let frac = src_pos - idx as f32;
            if idx + 1 < mono.len() {
                resampled.push(mono[idx] * (1.0 - frac) + mono[idx + 1] * frac);
            } else if idx < mono.len() {
                resampled.push(mono[idx]);
            }
        }
        Ok(resampled)
    }
}
