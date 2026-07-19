use std::path::PathBuf;

/// Generates and caches a 1-second 16kHz mono 16-bit PCM silence WAV file.
/// Used as a warm-up request to prevent server cold-start latency.
pub struct WakeWav {
    cached: Option<PathBuf>,
}

impl WakeWav {
    pub fn new() -> Self {
        Self { cached: None }
    }

    /// Ensure the silence WAV exists and return its path.
    pub fn ensure_silence_wav(&mut self) -> Result<PathBuf, String> {
        if let Some(ref cached) = self.cached {
            if cached.exists() {
                return Ok(cached.clone());
            }
        }

        let dir = std::env::temp_dir().join("voicepaste-wake");
        std::fs::create_dir_all(&dir).map_err(|e| format!("Cannot create dir: {}", e))?;
        let path = dir.join("silence-1s.wav");

        if path.exists() {
            self.cached = Some(path.clone());
            return Ok(path);
        }

        // Generate 1 second of 16kHz mono 16-bit silence
        let spec = hound::WavSpec {
            channels: 1,
            sample_rate: 16000,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };

        let mut writer = hound::WavWriter::create(&path, spec)
            .map_err(|e| format!("Cannot create WAV: {}", e))?;

        // 16000 samples of silence (1 second at 16kHz)
        for _ in 0..16000 {
            writer
                .write_sample(0i16)
                .map_err(|e| format!("Cannot write sample: {}", e))?;
        }

        writer
            .finalize()
            .map_err(|e| format!("Cannot finalize WAV: {}", e))?;

        self.cached = Some(path.clone());
        Ok(path)
    }
}
