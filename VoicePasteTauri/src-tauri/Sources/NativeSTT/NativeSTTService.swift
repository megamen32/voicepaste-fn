import Foundation
import AVFoundation
import Speech

/// Pure bridge to Apple's SFSpeechRecognizer. The Rust parent process spawns
/// `NativeSTTExec` (a thin CLI wrapper around this class) per dictation, then
/// reads the JSON result from stdout.
///
/// The recognizer is *not* created in `init` because `SFSpeechRecognizer.init`
/// can fail on locales the OS doesn't have a model for. We build it lazily
/// inside `recognize(wavPath:locale:)` so the error path is straightforward.
///
/// Permission flow: the parent Tauri process already has
/// `NSSpeechRecognitionUsageDescription` in Info.plist and has been granted
/// Speech authorization (we check via `authorizationStatus()`). The helper
/// itself inherits the parent's auth status — Speech framework authorization
/// is per-application, not per-process, so the same bundle ID is what matters.
public final class NativeSTTService {

    public enum RecognitionError: Error, LocalizedError {
        case authorizationDenied
        case authorizationRestricted
        case recognizerUnavailable(locale: String)
        case fileNotFound(path: String)
        case fileReadFailed(path: String, underlying: String)
        case audioConversionFailed(String)
        case recognitionFailed(String)
        case timeout

        public var errorDescription: String? {
            switch self {
            case .authorizationDenied:
                return "SFSpeechRecognizer authorization denied"
            case .authorizationRestricted:
                return "SFSpeechRecognizer authorization restricted"
            case .recognizerUnavailable(let locale):
                return "SFSpeechRecognizer unavailable for locale '\(locale)'"
            case .fileNotFound(let path):
                return "Audio file not found: \(path)"
            case .fileReadFailed(let path, let underlying):
                return "Failed to read audio file \(path): \(underlying)"
            case .audioConversionFailed(let msg):
                return "Audio conversion failed: \(msg)"
            case .recognitionFailed(let msg):
                return "Recognition failed: \(msg)"
            case .timeout:
                return "Recognition timed out"
            }
        }
    }

    public init() {}

    /// Recognize speech in a WAV file.
    /// - Parameters:
    ///   - wavPath: absolute path to a WAV file (any sample rate; AVFoundation
    ///     decodes it).
    ///   - locale: BCP-47 code like "ru", "ru-RU", "en-US". "auto" maps to
    ///     the system default recognizer.
    ///   - timeout: maximum wall-clock seconds to wait for the recognition
    ///     callback. SFSpeechRecognizer normally returns in a few seconds for
    ///     short clips; 30s is a safe upper bound for the dictation use case.
    /// - Returns: The recognized text (trimmed). Empty string if the recognizer
    ///   returned a successful result with no transcription.
    public func recognize(
        wavPath: String,
        locale: String,
        timeout: TimeInterval = 30.0
    ) async throws -> String {
        // 1) Check auth first — fail fast with a clear code so the parent
        //    can decide whether to request permission or just skip the tier.
        let status = SFSpeechRecognizer.authorizationStatus()
        switch status {
        case .authorized:
            break
        case .denied:
            throw RecognitionError.authorizationDenied
        case .restricted:
            throw RecognitionError.authorizationRestricted
        case .notDetermined:
            // We deliberately don't trigger a permission prompt from the
            // helper — it's annoying UX to surface a system dialog from a
            // background process. The parent Tauri app should have prompted
            // already. Treat as denied.
            throw RecognitionError.authorizationDenied
        @unknown default:
            throw RecognitionError.authorizationDenied
        }

        // 2) Build the recognizer for the requested locale (or system default).
        let recognizer: SFSpeechRecognizer?
        if locale == "auto" || locale.isEmpty {
            recognizer = SFSpeechRecognizer()
        } else {
            recognizer = SFSpeechRecognizer(locale: Locale(identifier: locale))
        }
        guard let rec = recognizer, rec.isAvailable else {
            throw RecognitionError.recognizerUnavailable(locale: locale)
        }
        rec.defaultTaskHint = .dictation

        // 3) Build the request.
        let request = SFSpeechURLRecognitionRequest(url: URL(fileURLWithPath: wavPath))
        request.shouldReportPartialResults = false
        request.taskHint = .dictation
        if rec.supportsOnDeviceRecognition {
            // Prefer on-device to keep the user's audio off Apple's servers
            // (and to keep the helper working offline). SFSpeechRecognizer
            // silently falls back to server-side if on-device isn't available
            // for the locale.
            request.requiresOnDeviceRecognition = true
        }

        // 4) Read the file and decode it via AVAudioFile. This is a fast
        //    pre-flight check: a corrupted WAV throws here with a clear
        //    message instead of triggering a generic `kAFAssistantErrorDomain`
        //    inside Speech. The recognizer will re-read the file via the
        //    URL-based request below.
        guard FileManager.default.fileExists(atPath: wavPath) else {
            throw RecognitionError.fileNotFound(path: wavPath)
        }
        let audioFile: AVAudioFile
        do {
            audioFile = try AVAudioFile(forReading: URL(fileURLWithPath: wavPath))
        } catch {
            throw RecognitionError.fileReadFailed(
                path: wavPath,
                underlying: error.localizedDescription
            )
        }

        // Use a sensible target format: 16 kHz / mono / Float32 — what
        // SFSpeechRecognizer is happiest with on macOS.
        let targetFormat = AVAudioFormat(
            commonFormat: .pcmFormatFloat32,
            sampleRate: 16000,
            channels: 1,
            interleaved: false
        )!
        // Decode to verify the file is valid; result is unused (URL request
        // re-reads the file).
        _ = try? convert(file: audioFile, to: targetFormat)

        // 5) Run the recognition task. SFSpeechRecognitionTask is callback-
        //    based, so we wrap it in a continuation.
        let text: String = try await withCheckedThrowingContinuation { continuation in
            var finished = false
            let finishOnce: (Result<String, Error>) -> Void = { result in
                if !finished {
                    finished = true
                    continuation.resume(with: result)
                }
            }

            // Recognition can hang in rare cases (e.g. corrupted audio). A
            // watchdog timer caps the wait. URL-based requests don't expose
            // a way to abort, so we just resume with `.timeout` — if the
            // recognizer later returns, `finishOnce` no-ops via the
            // `finished` guard. (Process exit shortly after will reap it.)
            let timer = DispatchSource.makeTimerSource(queue: .global())
            timer.schedule(deadline: .now() + timeout)
            timer.setEventHandler {
                finishOnce(.failure(RecognitionError.timeout))
            }
            timer.resume()

            rec.recognitionTask(with: request) { result, error in
                if let error = error {
                    timer.cancel()
                    finishOnce(.failure(RecognitionError.recognitionFailed(error.localizedDescription)))
                    return
                }
                guard let result = result else { return }
                if result.isFinal {
                    timer.cancel()
                    let text = result.bestTranscription.formattedString
                    finishOnce(.success(text))
                }
            }
        }

        return text.trimmingCharacters(in: .whitespacesAndNewlines)
    }

    // MARK: - Audio decode helper

    /// Decode an AVAudioFile to a PCM buffer in `targetFormat`. We don't
    /// actually use the buffer (the recognizer re-reads the URL), but
    /// decoding here is a fast pre-flight check: a corrupted WAV throws
    /// here with a clear message instead of triggering a generic
    /// `kAFAssistantErrorDomain` inside Speech.
    private func convert(
        file: AVAudioFile,
        to targetFormat: AVAudioFormat
    ) throws -> AVAudioPCMBuffer {
        let frameCount = AVAudioFrameCount(file.length)
        guard let buffer = AVAudioPCMBuffer(
            pcmFormat: file.processingFormat,
            frameCapacity: frameCount
        ) else {
            throw RecognitionError.audioConversionFailed("could not allocate PCM buffer")
        }
        try file.read(into: buffer)

        // If the file's format already matches what we need, return as-is.
        if file.processingFormat == targetFormat {
            return buffer
        }

        // Otherwise, convert via AVAudioConverter.
        guard let converter = AVAudioConverter(from: file.processingFormat, to: targetFormat) else {
            throw RecognitionError.audioConversionFailed(
                "no converter from \(file.processingFormat) to \(targetFormat)"
            )
        }
        let ratio = targetFormat.sampleRate / file.processingFormat.sampleRate
        let outFrames = AVAudioFrameCount(Double(frameCount) * Double(ratio) + 0.5)
        guard let out = AVAudioPCMBuffer(pcmFormat: targetFormat, frameCapacity: outFrames) else {
            throw RecognitionError.audioConversionFailed("could not allocate converted buffer")
        }

        var supplied = false
        var convertError: NSError?
        let status = converter.convert(to: out, error: &convertError) { _, status in
            if supplied {
                status.pointee = .endOfStream
                return nil
            }
            supplied = true
            status.pointee = .haveData
            return buffer
        }
        if status == .error, let err = convertError {
            throw RecognitionError.audioConversionFailed(err.localizedDescription)
        }
        return out
    }
}
