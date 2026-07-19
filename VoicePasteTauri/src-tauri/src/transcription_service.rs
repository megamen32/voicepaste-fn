use std::path::Path;

/// Abstracts any speech-to-text backend (server or local).
pub trait TranscriptionService: Send + Sync {
    /// Transcribe an audio file.
    /// - `file_path`: Path to the WAV file.
    /// - `language_code`: BCP-47 code (e.g. "ru", "en") or None for auto-detect.
    fn transcribe(&self, file_path: &Path, language_code: Option<&str>) -> Result<String, String>;
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
    pub fn transcribe(&self, file_path: &Path, language_code: Option<&str>) -> Result<String, String> {
        let mut last_error = String::new();

        for attempt in 1..=self.max_attempts {
            match self.primary.transcribe(file_path, language_code) {
                Ok(text) => return Ok(text),
                Err(e) => {
                    log::warn!("transcription attempt {}/{} failed: {}", attempt, self.max_attempts, e);
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
        self.transcriber.transcribe(file_path, lang, self.model.as_deref(), &self.config)
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
        fn transcribe(&self, _file_path: &Path, _language_code: Option<&str>) -> Result<String, String> {
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
    fn test_primary_success_first_attempt() {
        let (mock, count) = MockService::new(vec![Ok("hello".to_string())]);
        let retry = RetryTranscriber::new(Box::new(mock), None, 3);
        assert_eq!(retry.transcribe(dummy_path(), None).unwrap(), "hello");
        assert_eq!(count.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn test_primary_success_second_attempt() {
        let (mock, count) = MockService::new(vec![
            Err("fail".to_string()),
            Ok("hello".to_string()),
        ]);
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
        assert_eq!(retry.transcribe(dummy_path(), None).unwrap(), "fallback text");
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
        let (primary, pcount) = MockService::new(vec![
            Err("fail".to_string()),
            Ok("success".to_string()),
        ]);
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
            fn transcribe(&self, _file_path: &Path, language_code: Option<&str>) -> Result<String, String> {
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
}
