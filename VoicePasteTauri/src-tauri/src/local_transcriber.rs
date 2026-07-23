use crate::transcription_service::TranscriptionService;
use std::path::{Path, PathBuf};
use whisper_rs::{FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters};

/// Where VoicePaste expects whisper.cpp models to live. Used by both the
/// auto-discovery in `find_model()` and the tray "Open Models Folder" entry.
pub const DEFAULT_MODEL_FILENAME: &str = "ggml-base.bin";

/// Stable ids used by the Settings UI. Keep these independent from display
/// names so changing translations does not invalidate a user's config.
pub const LOCAL_MODEL_WHISPER_BASE: &str = "whisper-base";
pub const LOCAL_MODEL_PARAKEET_V3: &str = "parakeet-v3";

/// huggingface URL of the default `ggml-base` model (~140 MB). Used by the
/// tray "Download local model" entry.
pub const DEFAULT_MODEL_DOWNLOAD_URL: &str =
    "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-base.bin";

/// Parakeet is intentionally exposed as a provider instead of being compiled
/// into this binary. The NeMo runtime is large and platform-specific; users
/// can connect a local `parakeet-asr`/sherpa command from Settings.
pub const PARAKEET_V3_MODEL_URL: &str = "https://huggingface.co/nvidia/parakeet-tdt-0.6b-v3";

/// Resolve the directory where the model file should live.
/// Creates the directory if it doesn't exist.
pub fn models_dir() -> PathBuf {
    let dir = directories::ProjectDirs::from("com", "bezrabotnyi", "voicepaste")
        .map(|d| d.data_dir().to_path_buf())
        .unwrap_or_else(|| std::env::temp_dir().join("voicepaste-data"));
    let _ = std::fs::create_dir_all(&dir);
    dir
}

/// Full path to the default model file (where find_model() looks).
pub fn default_model_path() -> PathBuf {
    models_dir().join(DEFAULT_MODEL_FILENAME)
}

/// Lightweight status of the local whisper model, for UI / logging.
#[derive(Debug, Clone)]
pub enum ModelStatus {
    NotPresent,
    Present { path: PathBuf, bytes: u64 },
}

/// Download the default whisper model with progress logging.
///
/// Uses `curl` as a subprocess — no extra deps, streams stderr to the log
/// so the user can see bytes received as it goes. The model is ~140 MB so
/// we log a milestone every ~10 MB by watching the partial file size grow.
///
/// This is a *best-effort, fire-and-forget* download: the caller is expected
/// to spawn it on a background thread (the tray does this via
/// `std::thread::spawn`). On success, the file at `dest` is the full model
/// ready for `find_model()` to discover. On failure, the partial file is
/// kept (with a `.partial` suffix) so the user can resume by re-clicking
/// "Download local model".
///
/// `progress_cb` is called with the running byte count as the download
/// progresses. Keep it cheap (the tray just logs from it).
pub fn download_model_with_progress<F: FnMut(u64)>(
    url: &str,
    dest: &Path,
    mut progress_cb: F,
) -> Result<(), String> {
    use std::io::{BufRead, BufReader};
    use std::process::{Command, Stdio};
    use std::time::Duration;

    if let Some(parent) = dest.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let partial = dest.with_extension("bin.partial");

    // -s silent (no progress bar), -f fail on HTTP error,
    // --create-dirs to be safe, -C - to resume if a .partial exists.
    let mut child = Command::new("curl")
        .args([
            "-L", // follow redirects (huggingface redirects)
            "-f", // fail on HTTP >= 400
            "-s", // silence the progress bar
            "--create-dirs",
            "-C",
            "-", // resume
            "-o",
            partial.to_str().unwrap_or("voicepaste-model.bin.partial"),
            url,
        ])
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("Failed to spawn curl: {}", e))?;

    // Drain curl's stderr on a side thread so it can't block on a full pipe.
    // We log it at debug so the user can crank up RUST_LOG if they want to
    // diagnose a flaky network.
    let stderr_thread = child.stderr.take().map(|stderr| {
        std::thread::spawn(move || {
            let reader = BufReader::new(stderr);
            for line in reader.lines().map_while(Result::ok) {
                log::debug!("curl: {}", line);
            }
        })
    });

    // Poll partial file size for progress while curl runs.
    let mut last_logged_mb: u64 = 0;
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                if !status.success() {
                    if let Some(t) = stderr_thread {
                        let _ = t.join();
                    }
                    return Err(format!(
                        "curl exited with {} (partial file kept at {})",
                        status,
                        partial.display()
                    ));
                }
                break;
            }
            Ok(None) => {
                if let Ok(meta) = std::fs::metadata(&partial) {
                    let bytes = meta.len();
                    progress_cb(bytes);
                    let mb = bytes / (1024 * 1024);
                    if mb >= last_logged_mb + 10 {
                        last_logged_mb = mb;
                        log::info!("Downloading model: {} MB", mb);
                    }
                }
                std::thread::sleep(Duration::from_millis(250));
            }
            Err(e) => {
                if let Some(t) = stderr_thread {
                    let _ = t.join();
                }
                return Err(format!("try_wait failed: {}", e));
            }
        }
    }
    if let Some(t) = stderr_thread {
        let _ = t.join();
    }

    // Move .partial into place atomically.
    std::fs::rename(&partial, dest)
        .map_err(|e| format!("Failed to move model into place: {}", e))?;
    log::info!(
        "Model ready at {} ({} bytes). Click tray → STT Engine → Local to enable.",
        dest.display(),
        std::fs::metadata(dest).map(|m| m.len()).unwrap_or(0),
    );
    Ok(())
}

/// Quick HEAD request to discover the response Content-Length. Used by
/// `download_default_model` so it can pass a meaningful `(downloaded, total)`
/// to the tray progress UI. If the HEAD fails for any reason we return
/// `None` and the caller treats total as "unknown" (still works, just no
/// progress percentage).
fn head_content_length(url: &str) -> Option<u64> {
    use std::process::Command;
    let output = Command::new("curl")
        .args([
            "-sIL",
            "-o",
            "/dev/null",
            "-w",
            "%{http_code} %{size_download}",
            url,
        ])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let s = String::from_utf8(output.stdout).ok()?;
    // Format: "<code> <size>" — for HEAD curl often reports size_download=0
    // and the real length is in Content-Length header, so parse that next.
    let _ = s; // value not directly usable; the header parse below handles it.
    let header_output = Command::new("curl").args(["-sIL", url]).output().ok()?;
    let header_text = String::from_utf8(header_output.stdout).ok()?;
    for line in header_text.lines() {
        if let Some(rest) = line.strip_prefix("Content-Length:") {
            if let Ok(n) = rest.trim().parse::<u64>() {
                return Some(n);
            }
        }
        // HTTP/2 uses lowercase.
        if let Some(rest) = line.strip_prefix("content-length:") {
            if let Ok(n) = rest.trim().parse::<u64>() {
                return Some(n);
            }
        }
    }
    None
}

/// Download the default whisper model to its canonical location, calling
/// `progress_cb(downloaded, total)` periodically. `total` is `Some(n)` when
/// the server advertised a Content-Length, otherwise `None` (UI should
/// fall back to a spinner).
///
/// Thin wrapper over `download_model_with_progress` — adds the Content-Length
/// probe and the simpler signature the tray wants. The full model URL lives
/// in `DEFAULT_MODEL_DOWNLOAD_URL`; the destination is `default_model_path()`.
///
/// Returns the destination path on success (so the caller can show
/// "downloaded to /…/ggml-base.bin" in a toast).
pub fn download_default_model<F: FnMut(u64, Option<u64>)>(
    mut progress_cb: F,
) -> Result<PathBuf, String> {
    let url = DEFAULT_MODEL_DOWNLOAD_URL;
    let dest = default_model_path();
    let total = head_content_length(url);

    download_model_with_progress(url, &dest, |bytes| {
        progress_cb(bytes, total);
    })?;
    Ok(dest)
}

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
        Self::find_model_for(LOCAL_MODEL_WHISPER_BASE)
    }

    /// Find the selected local model. Whisper models are native `.bin` files;
    /// Parakeet is handled by the command provider and therefore has no file
    /// path requirement here.
    pub fn find_model_for(model: &str) -> Option<std::path::PathBuf> {
        if model == LOCAL_MODEL_PARAKEET_V3 {
            return None;
        }

        // Check env var first
        if let Ok(p) = std::env::var("WHISPER_MODEL_PATH") {
            let path = std::path::PathBuf::from(p);
            if path.exists() {
                return Some(path);
            }
        }

        // Canonical location (also created by models_dir() on first access).
        let canonical = default_model_path();
        if canonical.exists() {
            return Some(canonical);
        }

        // Backwards-compat: any *.bin in the models dir (user dropped a custom model there).
        if let Some(dir) = canonical.parent() {
            if let Ok(entries) = std::fs::read_dir(dir) {
                for entry in entries.flatten() {
                    let p = entry.path();
                    if p.extension().and_then(|s| s.to_str()) == Some("bin") {
                        return Some(p);
                    }
                }
            }
        }

        // Check current dir
        let local = std::path::PathBuf::from(DEFAULT_MODEL_FILENAME);
        if local.exists() {
            return Some(local);
        }

        None
    }

    /// Lightweight model status for the tray UI (no full I/O).
    pub fn model_status() -> ModelStatus {
        Self::model_status_for(LOCAL_MODEL_WHISPER_BASE)
    }

    /// Lightweight status for a selected provider.
    pub fn model_status_for(model: &str) -> ModelStatus {
        match Self::find_model_for(model) {
            None => ModelStatus::NotPresent,
            Some(path) => {
                let size = std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
                ModelStatus::Present { path, bytes: size }
            }
        }
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
        let wav_data =
            std::fs::read(file_path).map_err(|e| format!("Cannot read audio file: {}", e))?;

        let samples = wav_to_f32_samples(&wav_data)?;

        // Empty / silent recording precheck.
        //
        // When the user releases the hotkey very quickly (sub-200ms),
        // cpal finalizes a WAV that contains only the RIFF header with
        // zero PCM samples. Feeding an empty buffer to whisper.cpp
        // produces the cryptic error "Input sample buffer was empty"
        // and aborts the cascade before it can fall through to the
        // next tier. Catch it here instead, with a clean error the
        // cascade can log and continue past.
        if samples.is_empty() {
            log::info!(
                "LocalTranscriber: audio file {} contains 0 samples — skipping whisper.cpp",
                file_path.display()
            );
            return Err("empty audio input".to_string());
        }

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
        let lang_opt = language_code.filter(|l| *l != "auto");
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
    let mut reader =
        hound::WavReader::new(cursor).map_err(|e| format!("Invalid WAV file: {}", e))?;

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn model_status_default_is_not_present_on_clean_machine() {
        // On a clean machine there's no whisper model → status is NotPresent.
        // If a model IS present this returns Present (and we assert the path
        // is set) — either way the factory is correct.
        match LocalTranscriber::model_status() {
            ModelStatus::NotPresent => {} // expected
            ModelStatus::Present { path, .. } => {
                assert!(path.exists(), "Present status with non-existent path");
            }
        }
    }

    #[test]
    fn default_model_path_lives_in_models_dir() {
        let path = default_model_path();
        assert_eq!(
            path.file_name().and_then(|s| s.to_str()),
            Some(DEFAULT_MODEL_FILENAME)
        );
        assert_eq!(path.parent(), Some(models_dir().as_path()));
    }

    #[test]
    fn parakeet_provider_is_not_treated_as_a_whisper_file() {
        assert!(LocalTranscriber::find_model_for(LOCAL_MODEL_PARAKEET_V3).is_none());
        assert!(matches!(
            LocalTranscriber::model_status_for(LOCAL_MODEL_PARAKEET_V3),
            ModelStatus::NotPresent
        ));
    }

    #[test]
    fn download_default_model_signature_compiles() {
        // Signature smoke test: the function exists and accepts a
        // `(u64, Option<u64>)` callback. We don't actually call it —
        // that would trigger a 140 MB download. The tray integration
        // is the real end-to-end test.
        //
        // If this test compiles, the signature is correct. We just
        // take a function-item reference to force the compiler to
        // check the types.
        fn _check<F: FnMut(u64, Option<u64>)>() {
            let _f = download_default_model::<F>;
        }
    }

    /// Bug repro: when the user releases the hotkey very quickly (e.g.
    /// < 200ms), cpal finalizes a WAV with **zero** PCM samples — just
    /// the RIFF header. Without an empty-precheck, the call to
    /// `whisper_rs::WhisperContext::full` panics / returns the cryptic
    /// "Input sample buffer was empty" error. The cascade then can't
    /// fall through to the next tier cleanly.
    ///
    /// Expected behavior: `LocalTranscriber::transcribe` returns
    /// `Err("empty audio input")` BEFORE creating the whisper context,
    /// so the cascade logs a clean error and continues to the next tier.
    #[test]
    fn transcribe_empty_wav_returns_empty_error() {
        // Fake model file — the empty precheck should fire BEFORE whisper
        // actually tries to load it. We never want a real ~140 MB model
        // sitting in a unit test's temp dir.
        let model_path = std::env::temp_dir().join("voicepaste_test_fake_model.bin");
        std::fs::write(&model_path, b"").expect("write fake model");

        // Real empty WAV: 16kHz mono, 0 samples (header only, ~44 bytes).
        let wav_path = std::env::temp_dir().join("voicepaste_test_empty.wav");
        let spec = hound::WavSpec {
            channels: 1,
            sample_rate: 16000,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };
        let writer = hound::WavWriter::create(&wav_path, spec).expect("create empty WAV");
        writer.finalize().expect("finalize empty WAV");

        // Verify the test setup itself: the WAV must truly have 0 frames.
        let reader = hound::WavReader::open(&wav_path).expect("open empty WAV");
        assert_eq!(
            reader.duration(),
            0,
            "test setup invariant: WAV should have 0 frames"
        );

        let transcriber = LocalTranscriber::new(model_path.clone());
        let result = transcriber.transcribe(&wav_path, Some("ru"));

        // Cleanup regardless of result.
        let _ = std::fs::remove_file(&model_path);
        let _ = std::fs::remove_file(&wav_path);

        let err = result.expect_err("empty WAV should return Err, not Ok");
        assert!(
            err.to_lowercase().contains("empty"),
            "error message should mention 'empty', got: {}",
            err
        );
    }
}
