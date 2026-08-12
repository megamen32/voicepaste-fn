use std::path::Path;

/// Abstracts any speech-to-text backend (server or local).
pub trait TranscriptionService: Send + Sync {
    /// Transcribe an audio file.
    /// - `file_path`: Path to the WAV file.
    /// - `language_code`: BCP-47 code (e.g. "ru", "en") or None for auto-detect.
    fn transcribe(&self, file_path: &Path, language_code: Option<&str>) -> Result<String, String>;
}

/// Cascades through an ordered list of transcription services.
/// Tries each tier in order; returns Ok on the first success, or the LAST error
/// if every tier fails. By default, an empty `Ok("")` from any tier also stops
/// the cascade (silence is silence — no point burning whisper.cpp on it).
/// Set `stop_on_empty = false` to keep trying even after an empty result.
pub struct CascadeTranscriber {
    tiers: Vec<Box<dyn TranscriptionService>>,
    stop_on_empty: bool,
}

impl CascadeTranscriber {
    /// Create a new cascade. By default, empty results stop the cascade.
    pub fn new(tiers: Vec<Box<dyn TranscriptionService>>) -> Self {
        Self {
            tiers,
            stop_on_empty: true,
        }
    }

    /// Builder-style setter for `stop_on_empty`.
    pub fn with_stop_on_empty(mut self, stop_on_empty: bool) -> Self {
        self.stop_on_empty = stop_on_empty;
        self
    }

    /// Number of tiers in the cascade (useful for tests / diagnostics).
    pub fn len(&self) -> usize {
        self.tiers.len()
    }

    /// Is the cascade empty?
    pub fn is_empty(&self) -> bool {
        self.tiers.is_empty()
    }

    /// Try each tier in order. Returns the first Ok (including empty if
    /// `stop_on_empty` is false), or the last Err if all fail.
    pub fn transcribe(
        &self,
        file_path: &Path,
        language_code: Option<&str>,
    ) -> Result<String, String> {
        if self.tiers.is_empty() {
            return Err("no transcription tiers configured".to_string());
        }

        let mut last_error = String::new();
        let total = self.tiers.len();

        for (idx, tier) in self.tiers.iter().enumerate() {
            match tier.transcribe(file_path, language_code) {
                Ok(text) => {
                    if self.stop_on_empty && text.is_empty() {
                        log::warn!(
                            "cascade tier {}/{} ({}) returned empty — stopping cascade",
                            idx + 1,
                            total,
                            std::any::type_name_of_val(tier.as_ref())
                        );
                        return Ok(text);
                    }
                    if text.is_empty() {
                        // stop_on_empty=false: treat empty like a soft failure,
                        // keep going to the next tier.
                        log::warn!(
                            "cascade tier {}/{} ({}) returned empty — continuing (stop_on_empty=false)",
                            idx + 1,
                            total,
                            std::any::type_name_of_val(tier.as_ref())
                        );
                        last_error = "empty result".to_string();
                        continue;
                    }
                    return Ok(text);
                }
                Err(e) => {
                    log::warn!(
                        "cascade tier {}/{} ({}) failed: {}",
                        idx + 1,
                        total,
                        std::any::type_name_of_val(tier.as_ref()),
                        e
                    );
                    last_error = e;
                }
            }
        }

        Err(if last_error.is_empty() {
            "All transcription tiers failed".to_string()
        } else {
            last_error
        })
    }
}

/// Orchestrates server transcription with automatic retries and optional local fallback.
pub struct RetryTranscriber {
    primary: Box<dyn TranscriptionService>,
    fallback: Option<Box<dyn TranscriptionService>>,
    max_attempts: usize,
}

impl RetryTranscriber {
    pub fn new(
        primary: Box<dyn TranscriptionService>,
        fallback: Option<Box<dyn TranscriptionService>>,
        max_attempts: usize,
    ) -> Self {
        Self {
            primary,
            fallback,
            max_attempts: max_attempts.max(1),
        }
    }

    /// Attempt transcription up to `max_attempts` times with the primary service.
    /// If all attempts fail and a fallback is configured, try the fallback once.
    /// Returns the last primary error if everything fails.
    pub fn transcribe(
        &self,
        file_path: &Path,
        language_code: Option<&str>,
    ) -> Result<String, String> {
        let mut last_error = String::new();

        for attempt in 1..=self.max_attempts {
            match self.primary.transcribe(file_path, language_code) {
                Ok(text) => return Ok(text),
                Err(e) => {
                    log::warn!(
                        "transcription attempt {}/{} failed: {}",
                        attempt,
                        self.max_attempts,
                        e
                    );
                    last_error = e;
                }
            }
        }

        // All primary attempts failed — try fallback
        if let Some(ref fallback) = self.fallback {
            match fallback.transcribe(file_path, language_code) {
                Ok(text) => return Ok(text),
                Err(e) => {
                    log::warn!("fallback transcription failed: {}", e);
                }
            }
        }

        Err(if last_error.is_empty() {
            "All transcription attempts failed".to_string()
        } else {
            last_error
        })
    }
}

/// Adapter that wraps `Transcriber` to implement `TranscriptionService`.
pub struct ServerTranscriptionService {
    transcriber: crate::transcriber::Transcriber,
    config: crate::config::AppConfig,
    language: crate::models::Language,
    model: Option<String>,
}

impl ServerTranscriptionService {
    pub fn new(
        transcriber: crate::transcriber::Transcriber,
        config: crate::config::AppConfig,
        language: crate::models::Language,
        model: Option<String>,
    ) -> Self {
        Self {
            transcriber,
            config,
            language,
            model,
        }
    }
}

impl TranscriptionService for ServerTranscriptionService {
    fn transcribe(&self, file_path: &Path, language_code: Option<&str>) -> Result<String, String> {
        let lang = language_code
            .and_then(|code| match code {
                "ru" => Some(crate::models::Language::Ru),
                "en" => Some(crate::models::Language::En),
                _ => None,
            })
            .unwrap_or(self.language);
        self.transcriber
            .transcribe(file_path, lang, self.model.as_deref(), &self.config)
    }
}

/// Runs a local STT command using the same small provider contract used by
/// Hermes: `{input_path}`, `{output_path}`, and `{language}` placeholders.
/// The command may write plain text to `output_path`; stdout is accepted as a
/// fallback so simple CLI tools work too.
pub struct CommandTranscriptionService {
    command_template: String,
    model_dir: Option<std::path::PathBuf>,
}

impl CommandTranscriptionService {
    pub fn new(command_template: impl Into<String>) -> Self {
        Self {
            command_template: command_template.into(),
            model_dir: None,
        }
    }

    pub fn with_model_dir(mut self, model_dir: impl Into<std::path::PathBuf>) -> Self {
        self.model_dir = Some(model_dir.into());
        self
    }

    fn render_command(
        &self,
        input_path: &Path,
        output_path: &Path,
        language_code: Option<&str>,
    ) -> String {
        let input = shell_quote(input_path.to_string_lossy().as_ref());
        let output = shell_quote(output_path.to_string_lossy().as_ref());
        let model_dir = self
            .model_dir
            .as_deref()
            .map(|path| shell_quote(path.to_string_lossy().as_ref()))
            .unwrap_or_default();
        self.command_template
            .replace("{input_path}", &input)
            .replace("{output_path}", &output)
            .replace("{model_dir}", &model_dir)
            .replace("{model_path}", &model_dir)
            .replace("{language}", language_code.unwrap_or("auto"))
    }
}

impl TranscriptionService for CommandTranscriptionService {
    fn transcribe(&self, file_path: &Path, language_code: Option<&str>) -> Result<String, String> {
        if self.command_template.trim().is_empty() {
            return Err("local command provider is not configured".to_string());
        }

        let output_path = std::env::temp_dir().join(format!(
            "voicepaste-stt-{}-{}.txt",
            std::process::id(),
            chrono::Utc::now().timestamp_nanos_opt().unwrap_or_default()
        ));
        let command_line = self.render_command(file_path, &output_path, language_code);

        #[cfg(target_os = "windows")]
        let result = std::process::Command::new("cmd")
            .args(["/C", &command_line])
            .output();
        #[cfg(not(target_os = "windows"))]
        let result = std::process::Command::new("sh")
            .args(["-c", &command_line])
            .output();

        let output = result.map_err(|e| format!("failed to start local STT command: {}", e))?;
        let text = std::fs::read_to_string(&output_path)
            .ok()
            .or_else(|| String::from_utf8(output.stdout).ok())
            .unwrap_or_default()
            .trim()
            .to_string();
        let _ = std::fs::remove_file(&output_path);

        if !output.status.success() {
            let details = String::from_utf8_lossy(&output.stderr).trim().to_string();
            return Err(if details.is_empty() {
                format!("local STT command exited with {}", output.status)
            } else {
                format!("local STT command failed: {}", details)
            });
        }
        if text.is_empty() {
            return Err("local STT command returned empty result".to_string());
        }
        Ok(text)
    }
}

fn shell_quote(value: &str) -> String {
    #[cfg(target_os = "windows")]
    {
        format!("\"{}\"", value.replace('"', "\\\""))
    }
    #[cfg(not(target_os = "windows"))]
    {
        format!("'{}'", value.replace('\'', "'\\''"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;

    struct MockService {
        results: Vec<Result<String, String>>,
        call_count: Arc<AtomicUsize>,
    }

    impl MockService {
        fn new(results: Vec<Result<String, String>>) -> (Self, Arc<AtomicUsize>) {
            let count = Arc::new(AtomicUsize::new(0));
            (
                Self {
                    results,
                    call_count: count.clone(),
                },
                count,
            )
        }
    }

    impl TranscriptionService for MockService {
        fn transcribe(
            &self,
            _file_path: &Path,
            _language_code: Option<&str>,
        ) -> Result<String, String> {
            let idx = self.call_count.fetch_add(1, Ordering::SeqCst);
            if idx < self.results.len() {
                self.results[idx].clone()
            } else {
                Err("unexpected call".to_string())
            }
        }
    }

    fn dummy_path() -> &'static Path {
        Path::new("/tmp/test.wav")
    }

    #[test]
    fn command_provider_renders_all_placeholders() {
        let provider = CommandTranscriptionService::new(
            "parakeet --in {input_path} --out {output_path} --lang {language}",
        );
        let rendered = provider.render_command(
            Path::new("/tmp/input voice.wav"),
            Path::new("/tmp/output.txt"),
            Some("ru"),
        );
        assert!(rendered.contains("/tmp/input voice.wav"));
        assert!(rendered.contains("/tmp/output.txt"));
        assert!(rendered.ends_with("--lang ru"));
    }

    #[test]
    fn command_provider_runs_a_cross_platform_stdout_command() {
        #[cfg(target_os = "windows")]
        let command = "echo hello";
        #[cfg(not(target_os = "windows"))]
        let command = "printf hello";

        let provider = CommandTranscriptionService::new(command);
        assert_eq!(
            provider.transcribe(dummy_path(), Some("en")).unwrap(),
            "hello"
        );
    }

    #[test]
    fn test_primary_success_first_attempt() {
        let (mock, count) = MockService::new(vec![Ok("hello".to_string())]);
        let retry = RetryTranscriber::new(Box::new(mock), None, 3);
        assert_eq!(retry.transcribe(dummy_path(), None).unwrap(), "hello");
        assert_eq!(count.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn test_primary_success_second_attempt() {
        let (mock, count) =
            MockService::new(vec![Err("fail".to_string()), Ok("hello".to_string())]);
        let retry = RetryTranscriber::new(Box::new(mock), None, 3);
        assert_eq!(retry.transcribe(dummy_path(), None).unwrap(), "hello");
        assert_eq!(count.load(Ordering::SeqCst), 2);
    }

    #[test]
    fn test_primary_success_third_attempt() {
        let (mock, count) = MockService::new(vec![
            Err("fail1".to_string()),
            Err("fail2".to_string()),
            Ok("hello".to_string()),
        ]);
        let retry = RetryTranscriber::new(Box::new(mock), None, 3);
        assert_eq!(retry.transcribe(dummy_path(), None).unwrap(), "hello");
        assert_eq!(count.load(Ordering::SeqCst), 3);
    }

    #[test]
    fn test_all_fail_no_fallback() {
        let (mock, _) = MockService::new(vec![
            Err("fail1".to_string()),
            Err("fail2".to_string()),
            Err("fail3".to_string()),
        ]);
        let retry = RetryTranscriber::new(Box::new(mock), None, 3);
        let err = retry.transcribe(dummy_path(), None).unwrap_err();
        assert_eq!(err, "fail3");
    }

    #[test]
    fn test_all_fail_with_fallback_success() {
        let (primary, _) = MockService::new(vec![
            Err("fail1".to_string()),
            Err("fail2".to_string()),
            Err("fail3".to_string()),
        ]);
        let (fallback, count) = MockService::new(vec![Ok("fallback text".to_string())]);
        let retry = RetryTranscriber::new(Box::new(primary), Some(Box::new(fallback)), 3);
        assert_eq!(
            retry.transcribe(dummy_path(), None).unwrap(),
            "fallback text"
        );
        assert_eq!(count.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn test_all_fail_fallback_also_fails() {
        let (primary, _) = MockService::new(vec![
            Err("primary_err".to_string()),
            Err("primary_err".to_string()),
            Err("primary_err".to_string()),
        ]);
        let (fallback, _) = MockService::new(vec![Err("fallback_err".to_string())]);
        let retry = RetryTranscriber::new(Box::new(primary), Some(Box::new(fallback)), 3);
        let err = retry.transcribe(dummy_path(), None).unwrap_err();
        assert_eq!(err, "primary_err");
    }

    #[test]
    fn test_success_before_fallback_not_called() {
        let (primary, pcount) =
            MockService::new(vec![Err("fail".to_string()), Ok("success".to_string())]);
        let (fallback, fcount) = MockService::new(vec![Ok("fallback".to_string())]);
        let retry = RetryTranscriber::new(Box::new(primary), Some(Box::new(fallback)), 3);
        assert_eq!(retry.transcribe(dummy_path(), None).unwrap(), "success");
        assert_eq!(pcount.load(Ordering::SeqCst), 2);
        assert_eq!(fcount.load(Ordering::SeqCst), 0); // fallback never called
    }

    #[test]
    fn test_max_attempts_one() {
        let (primary, _) = MockService::new(vec![Err("fail".to_string())]);
        let (fallback, count) = MockService::new(vec![Ok("fb".to_string())]);
        let retry = RetryTranscriber::new(Box::new(primary), Some(Box::new(fallback)), 1);
        assert_eq!(retry.transcribe(dummy_path(), None).unwrap(), "fb");
        assert_eq!(count.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn test_language_code_passthrough() {
        struct CapturingService {
            captured: Arc<parking_lot::Mutex<Option<String>>>,
        }
        impl TranscriptionService for CapturingService {
            fn transcribe(
                &self,
                _file_path: &Path,
                language_code: Option<&str>,
            ) -> Result<String, String> {
                *self.captured.lock() = language_code.map(String::from);
                Ok("ok".to_string())
            }
        }

        let captured = Arc::new(parking_lot::Mutex::new(None));
        let service = CapturingService {
            captured: captured.clone(),
        };
        let retry = RetryTranscriber::new(Box::new(service), None, 3);
        let _ = retry.transcribe(dummy_path(), Some("ru"));
        assert_eq!(*captured.lock(), Some("ru".to_string()));
    }

    // ------------------------------------------------------------------
    // CascadeTranscriber tests
    // ------------------------------------------------------------------

    /// Build a single-shot mock that returns `result` once and then panics
    /// on subsequent calls. Used to assert that a tier was NOT invoked.
    fn one_shot(
        result: Result<String, String>,
    ) -> (Box<dyn TranscriptionService>, Arc<AtomicUsize>) {
        struct OneShot {
            result: Result<String, String>,
            fired: Arc<AtomicUsize>,
        }
        impl TranscriptionService for OneShot {
            fn transcribe(&self, _p: &Path, _l: Option<&str>) -> Result<String, String> {
                self.fired.fetch_add(1, Ordering::SeqCst);
                self.result.clone()
            }
        }
        let fired = Arc::new(AtomicUsize::new(0));
        let svc = OneShot {
            result,
            fired: fired.clone(),
        };
        (Box::new(svc), fired)
    }

    #[test]
    fn test_cascade_tries_in_order() {
        let (t1, c1) = one_shot(Err("t1 fail".to_string()));
        let (t2, c2) = one_shot(Err("t2 fail".to_string()));
        let (t3, c3) = one_shot(Ok("t3 win".to_string()));
        let cascade = CascadeTranscriber::new(vec![t1, t2, t3]);
        let result = cascade.transcribe(dummy_path(), None).unwrap();
        assert_eq!(result, "t3 win");
        assert_eq!(c1.load(Ordering::SeqCst), 1);
        assert_eq!(c2.load(Ordering::SeqCst), 1);
        assert_eq!(c3.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn test_cascade_stops_on_empty_by_default() {
        // Tier 1 returns Ok("") — cascade should stop, tier 2 must NOT be called.
        let (t1, c1) = one_shot(Ok(String::new()));
        let (t2, c2) = one_shot(Ok("t2 should not run".to_string()));
        let cascade = CascadeTranscriber::new(vec![t1, t2]);
        let result = cascade.transcribe(dummy_path(), None).unwrap();
        assert_eq!(result, "");
        assert_eq!(c1.load(Ordering::SeqCst), 1);
        assert_eq!(c2.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn test_cascade_continues_on_empty_when_disabled() {
        // Same setup, but stop_on_empty=false — tier 2 SHOULD be called.
        let (t1, c1) = one_shot(Ok(String::new()));
        let (t2, c2) = one_shot(Ok("t2 ran".to_string()));
        let cascade = CascadeTranscriber::new(vec![t1, t2]).with_stop_on_empty(false);
        let result = cascade.transcribe(dummy_path(), None).unwrap();
        assert_eq!(result, "t2 ran");
        assert_eq!(c1.load(Ordering::SeqCst), 1);
        assert_eq!(c2.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn test_cascade_empty_tiers_returns_error() {
        let cascade = CascadeTranscriber::new(vec![]);
        let err = cascade.transcribe(dummy_path(), None).unwrap_err();
        assert_eq!(err, "no transcription tiers configured");
        assert!(cascade.is_empty());
        assert_eq!(cascade.len(), 0);
    }

    #[test]
    fn test_cascade_logs_each_attempt() {
        // 3 failing tiers — every one should have been called exactly once,
        // and the LAST error is returned (proves we logged and continued).
        let (t1, c1) = one_shot(Err("first fail".to_string()));
        let (t2, c2) = one_shot(Err("second fail".to_string()));
        let (t3, c3) = one_shot(Err("third fail".to_string()));
        let cascade = CascadeTranscriber::new(vec![t1, t2, t3]);
        let err = cascade.transcribe(dummy_path(), None).unwrap_err();
        assert_eq!(err, "third fail");
        assert_eq!(c1.load(Ordering::SeqCst), 1);
        assert_eq!(c2.load(Ordering::SeqCst), 1);
        assert_eq!(c3.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn test_cascade_all_fail_returns_last_error() {
        // Mirrors the OLD `RetryTranscriber` behavior on full failure but with 3 tiers.
        let (t1, _) = one_shot(Err("e1".to_string()));
        let (t2, _) = one_shot(Err("e2".to_string()));
        let (t3, _) = one_shot(Err("e3".to_string()));
        let cascade = CascadeTranscriber::new(vec![t1, t2, t3]);
        let err = cascade.transcribe(dummy_path(), None).unwrap_err();
        assert_eq!(err, "e3");
    }
}
