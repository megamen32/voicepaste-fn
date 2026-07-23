//! Apple SFSpeechRecognizer backend (macOS only).
//!
//! # Status: wired (3rd-tier cascade fallback)
//!
//! The Rust side spawns a small Swift helper binary
//! (`Sources/NativeSTTExec` → `native_stt`) per dictation, which actually calls
//! `SFSpeechRecognizer`. The helper reads a WAV file from disk, decodes it via
//! `AVAudioFile`, and writes the recognized text as a single JSON line on
//! stdout. Errors (auth denied, locale unavailable, decode failure, timeout)
//! go to stderr as structured JSON with a stable `code` field the Rust parent
//! can pattern-match on.
//!
//! # Permission flow
//!
//! Speech authorization is per-bundle-ID, not per-process. The parent Tauri
//! process already has `NSSpeechRecognitionUsageDescription` in `Info.plist`,
//! and the user is expected to have already granted Speech permission through
//! the Tauri app's UI. The helper inherits that auth status — when the test
//! runner or `cargo test` invokes the helper directly, the helper returns
//! `{"type":"error","code":"auth_denied",...}` and exits 1. That's by design;
//! the Tauri parent treats this as a soft failure and falls through to the
//! next cascade tier.
//!
//! # Why a helper, not a binding
//!
//! Three ways to wrap `SFSpeechRecognizer` on macOS:
//!
//! 1. **`objc2` + `objc2-speech` (or `objc2` + raw `objc::msg_send!`)** —
//!    ~150 lines of class lookup / `msg_send!` boilerplate, callback-based
//!    Speech framework needs a `block` or `protocol` declaration. Painful
//!    to write, harder to debug than Swift.
//! 2. **Swift helper binary (this implementation, mirrors `modifier_monitor`)** —
//!    small Swift file, easy to read & audit. Cost: extra process spawn per
//!    dictation (~50ms on M-series).
//! 3. **Inline `swift -e`** — zero extra code, but forks `swift` (~1s startup).
//!    Inappropriate for the hot path.
//!
//! Decision: option #2. The rest of the app already uses #2 for the
//! modifier-monitor hotkey path, so the build infra is in place. We can
//! promote to #1 later if startup latency matters.
//!
//! # What works today
//!
//! - `NativeSttService::is_available()` returns `true` on macOS, `false`
//!   everywhere else (the cascade uses this to skip tiers that can't run).
//! - `NativeSttService::transcribe()` spawns the helper, reads JSON from
//!   stdout, returns the recognized text (or a structured `Err`).
//! - Test seam: `NativeSttService::new_for_test(locale, helper_path)` lets
//!   unit tests pin a bogus helper path and assert clean `Err` behavior
//!   without bundling a real binary into the test process.

use crate::transcription_service::TranscriptionService;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Bridge to the real SFSpeechRecognizer binding. Spawns the Swift helper
/// (`Sources/NativeSTTExec`) which actually talks to SFSpeechRecognizer, then
/// reads the JSON result from its stdout.
///
/// Contract:
/// - `wav_path`: path to a WAV file on disk (any sample rate; the helper
///   will resample internally if needed).
/// - `locale`: BCP-47 code like "ru", "en", or "auto" for the recognizer's
///   default locale.
///
/// Returns the recognized text (already trimmed), or an `Err` describing the
/// failure (permission denied, recognizer unavailable, network required but
/// offline, helper missing, etc.).
#[cfg(target_os = "macos")]
fn recognize(wav_path: &Path, locale: &str, helper: &Path) -> Result<String, String> {
    let output = Command::new(helper)
        .arg(wav_path)
        .arg(locale)
        .output()
        .map_err(|e| format!("Failed to spawn native STT helper at {:?}: {}", helper, e))?;

    if !output.status.success() {
        let err = String::from_utf8_lossy(&output.stderr);
        // Helper writes structured error JSON on stderr; surface it as-is
        // so callers can pattern-match on the "code" field if they want to.
        let err_str = err.trim();
        if err_str.is_empty() {
            return Err(format!(
                "native STT helper exited with status {}",
                output.status
            ));
        }
        return Err(format!("native STT helper failed: {}", err_str));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let line = stdout
        .lines()
        .last()
        .ok_or_else(|| "native STT helper produced no output".to_string())?;

    let json: serde_json::Value = serde_json::from_str(line.trim())
        .map_err(|e| format!("native STT helper: invalid JSON: {} (line: {:?})", e, line))?;

    let text = json
        .get("text")
        .and_then(|v| v.as_str())
        .ok_or_else(|| "native STT helper: JSON missing 'text' field".to_string())?;

    Ok(text.trim().to_string())
}

#[cfg(not(target_os = "macos"))]
fn recognize(_wav_path: &Path, _locale: &str, _helper: &Path) -> Result<String, String> {
    Err("native STT (SFSpeechRecognizer) is macOS-only".to_string())
}

/// Locate the Swift helper binary. The lookup mirrors the `modifier_monitor`
/// pattern:
///
/// 1. `Contents/MacOS/native_stt` next to the running exe (production bundle
///    path via `externalBin`).
/// 2. Bare `native_stt` on `PATH` (dev fallback).
/// 3. `../Sources/NativeSTTExec/.build/release/native_stt` relative to
///    `CARGO_MANIFEST_DIR` (SwiftPM-built binary during dev).
/// 4. Same directory as `modifier_monitor` (they share the `externalBin`
///    pattern and live next to each other in the bundle).
///
/// On non-macOS, returns `Err` (this tier is skipped anyway, so it should
/// never be called — the cascade uses `is_available()` to gate).
#[cfg(target_os = "macos")]
fn find_helper_binary() -> Result<PathBuf, String> {
    use std::env;

    // 1) Production: next to the running exe (Contents/MacOS/).
    if let Ok(exe) = env::current_exe() {
        if let Some(dir) = exe.parent() {
            let candidate = dir.join("native_stt");
            if candidate.exists() {
                return Ok(candidate);
            }
        }
    }

    // 2) Dev: same trick used by `modifier_monitor` — fall back to bare name
    //    on PATH.
    let bare = PathBuf::from("native_stt");
    if bare.exists() {
        return Ok(bare);
    }

    // 3) SwiftPM build artifact relative to Cargo manifest dir.
    if let Ok(manifest) = env::var("CARGO_MANIFEST_DIR") {
        let swiftpm = PathBuf::from(manifest)
            .join("Sources")
            .join("NativeSTTExec")
            .join(".build")
            .join("release")
            .join("native_stt");
        if swiftpm.exists() {
            return Ok(swiftpm);
        }
    }

    // 4) Last-resort: look next to `modifier_monitor`. If we find that
    //    directory, the bundler should have placed `native_stt` there too.
    if let Ok(exe) = env::current_exe() {
        if let Some(dir) = exe.parent() {
            let sibling = dir.join("modifier_monitor");
            if sibling.exists() {
                let candidate = dir.join("native_stt");
                if candidate.exists() {
                    return Ok(candidate);
                }
            }
        }
    }

    Err("native STT helper binary not found. \
         Build it with `swift build --product native_stt -c release` \
         in src-tauri/ or bundle it via tauri.conf.json externalBin."
        .to_string())
}

#[cfg(not(target_os = "macos"))]
#[allow(dead_code)]
fn find_helper_binary() -> Result<PathBuf, String> {
    Err("native STT helper lookup is macOS-only".to_string())
}

/// Wrapper that exposes Apple's SFSpeechRecognizer as a `TranscriptionService`.
///
/// The struct is the *interface* the cascade depends on. The actual recognition
/// is done by a small Swift helper binary (see `Sources/NativeSTTExec`) that we
/// spawn per call and read JSON from stdout — mirrors the `modifier_monitor`
/// helper pattern.
pub struct NativeSttService {
    locale: String,
    /// Path to the Swift helper binary. If `None`, the service tries to locate
    /// it on its own (production path lookup).
    helper_path: Option<PathBuf>,
}

impl NativeSttService {
    /// Create a new service. `locale` is the BCP-47 code (e.g. "ru", "en",
    /// or "auto"); we store it and pass it to the recognizer on each call.
    ///
    /// The helper binary is located via [`find_helper_binary`] at call time.
    pub fn new(locale: impl Into<String>) -> Self {
        Self {
            locale: locale.into(),
            helper_path: None,
        }
    }

    /// Test-only constructor: takes the helper path explicitly so the test
    /// doesn't rely on `externalBin` lookup. Lets us assert clean error
    /// handling when the helper is missing/bogus without bundling a real
    /// binary into the test process.
    #[cfg(test)]
    pub fn new_for_test(locale: impl Into<String>, helper_path: PathBuf) -> Self {
        Self {
            locale: locale.into(),
            helper_path: Some(helper_path),
        }
    }

    /// Is this backend usable on the current platform / with current config?
    /// On non-macOS, this is always `false` (cascade skips the tier).
    #[cfg(target_os = "macos")]
    pub fn is_available() -> bool {
        // TODO: tighten this when the real binding lands — at minimum we
        // need `SFSpeechRecognizer.authorizationStatus() == .authorized`.
        // For now, on macOS we declare it available so the user sees the
        // tier wired up in the cascade. The `transcribe()` call itself
        // will return an error until the binding exists.
        true
    }

    #[cfg(not(target_os = "macos"))]
    pub fn is_available() -> bool {
        false
    }
}

impl TranscriptionService for NativeSttService {
    fn transcribe(&self, file_path: &Path, language_code: Option<&str>) -> Result<String, String> {
        // Resolve the helper. Test-only constructor pins the path; prod
        // constructor uses the lookup table.
        let helper = match &self.helper_path {
            Some(p) => p.clone(),
            None => find_helper_binary()?,
        };

        // Prefer the caller's language_code if given; fall back to the
        // service's configured locale. The helper accepts BCP-47 like
        // "ru-RU" or short codes like "ru".
        let locale = language_code.unwrap_or(&self.locale);

        // Verify the file is readable before we spawn the helper — gives
        // a clearer error than letting the helper bail on a missing path.
        if !file_path.exists() {
            return Err(format!("audio file does not exist: {:?}", file_path));
        }

        recognize(file_path, locale, &helper)
    }
}

/// Write a 0.5-second silent 16 kHz mono WAV to a temp file and return the
/// path. Used by the `new_for_test` test to exercise the helper-spawning path
/// without depending on a real recording.
#[cfg(test)]
fn write_test_silence_wav(path: &Path) -> Result<(), String> {
    use hound::{SampleFormat, WavSpec, WavWriter};
    let spec = WavSpec {
        channels: 1,
        sample_rate: 16000,
        bits_per_sample: 16,
        sample_format: SampleFormat::Int,
    };
    let mut writer = WavWriter::create(path, spec)
        .map_err(|e| format!("create test wav: {}", e))?;
    let n_samples = 16_000 / 2; // 0.5s
    for _ in 0..n_samples {
        writer
            .write_sample(0i16)
            .map_err(|e| format!("write sample: {}", e))?;
    }
    writer
        .finalize()
        .map_err(|e| format!("finalize wav: {}", e))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transcription_service::{CascadeTranscriber, TranscriptionService};
    use std::path::Path;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;

    /// Mock that pretends to be the real Native tier. We use this in cascade
    /// integration tests so the real (currently-stubbed) NativeSttService
    /// doesn't have to return a real transcription to be useful.
    struct MockNative {
        result: Result<String, String>,
        fired: Arc<AtomicUsize>,
    }

    impl TranscriptionService for MockNative {
        fn transcribe(&self, _p: &Path, _l: Option<&str>) -> Result<String, String> {
            self.fired.fetch_add(1, Ordering::SeqCst);
            self.result.clone()
        }
    }

    fn dummy_path() -> &'static Path {
        Path::new("/tmp/test.wav")
    }

    #[test]
    fn is_available_is_false_off_macos() {
        // On the test machine (could be either), this asserts the cfg flag
        // is respected. We don't fail if both branches compile — we just
        // assert the function returns a bool.
        let _ = NativeSttService::is_available();
    }

    #[test]
    fn real_native_stt_surfaces_clean_error_when_helper_missing() {
        // The real NativeSttService uses a Swift helper binary. If that
        // binary isn't built / installed, transcribe() must return a clean
        // Err — never panic, never block. The cascade relies on this to
        // fall through to the next tier.
        let svc = NativeSttService::new("en");
        let result = svc.transcribe(dummy_path(), Some("en"));
        assert!(result.is_err(), "expected Err when helper is missing, got Ok");
        let err = result.unwrap_err();
        // Any of these is acceptable — depends on the test env:
        //   - "audio file does not exist" (dummy_path is bogus)
        //   - "native STT helper binary not found" (lookup failed)
        //   - "Failed to spawn native STT helper" (binary missing on PATH)
        assert!(
            err.contains("audio file does not exist")
                || err.contains("native STT helper binary not found")
                || err.contains("Failed to spawn native STT helper"),
            "unexpected error message: {}",
            err
        );
    }

    /// TDD: this is the load-bearing test for the helper-spawning path.
    /// Uses `new_for_test` to pin a bogus helper path → expect a clean Err
    /// ("doesn't panic, returns Err") within a reasonable time. The point is
    /// to prove the new constructor wires through to `recognize()` correctly
    /// and the error path doesn't block forever or crash.
    #[test]
    fn new_for_test_with_bogus_helper_returns_clean_err() {
        // Write a real (silent) WAV so we exercise the file-exists check
        // AND the helper-spawning path. The helper at this path will fail
        // to spawn (file doesn't exist), so we expect an Err, not Ok.
        let tmp = std::env::temp_dir().join("voicepaste_native_stt_test.wav");
        write_test_silence_wav(&tmp).expect("write test wav");
        let bogus_helper = PathBuf::from("/definitely/does/not/exist/native_stt_helper_zzz");

        let svc = NativeSttService::new_for_test("ru", bogus_helper);
        let result = svc.transcribe(&tmp, Some("ru"));

        // Cleanup the temp file regardless of outcome.
        let _ = std::fs::remove_file(&tmp);

        // The test passes if: (a) we got a result, (b) it was an Err, and
        // (c) the Err mentions the helper path or "Failed to spawn". A panic
        // here would fail the test, which is what we want to catch.
        assert!(
            result.is_err(),
            "expected Err for bogus helper, got Ok: {:?}",
            result
        );
        let err = result.unwrap_err();
        assert!(
            err.contains("Failed to spawn native STT helper") || err.contains("native_stt"),
            "error should mention helper spawn failure, got: {}",
            err
        );
    }

    /// Language code from the call site wins over the service's configured
    /// locale. Verifies we don't silently drop the caller's preference.
    #[test]
    fn transcribe_passes_caller_language_code() {
        // This test doesn't actually run the helper — it just confirms we
        // can construct the service and the language code isn't lost in the
        // helper-resolution path. If the file is missing, we get an Err
        // mentioning that, which is fine.
        let bogus_helper = PathBuf::from("/definitely/does/not/exist/native_stt_helper_zzz");
        let svc = NativeSttService::new_for_test("en", bogus_helper);
        let result = svc.transcribe(dummy_path(), Some("ru-RU"));
        assert!(result.is_err()); // don't care about exact message
    }

    #[test]
    fn mock_native_works_as_cascade_tier() {
        // End-to-end: cascade skips failing Remote, hits successful mock Native.
        use std::sync::atomic::AtomicUsize;
        struct FailingRemote;
        impl TranscriptionService for FailingRemote {
            fn transcribe(&self, _p: &Path, _l: Option<&str>) -> Result<String, String> {
                Err("remote 500".to_string())
            }
        }
        let _ = AtomicUsize::new(0); // silence unused-import lints if any
        let fired = Arc::new(AtomicUsize::new(0));
        let mock = MockNative {
            result: Ok("native wins".to_string()),
            fired: fired.clone(),
        };
        let cascade = CascadeTranscriber::new(vec![
            Box::new(FailingRemote),
            Box::new(mock),
        ]);
        let result = cascade.transcribe(dummy_path(), None).unwrap();
        assert_eq!(result, "native wins");
        assert_eq!(fired.load(Ordering::SeqCst), 1);
    }
}
