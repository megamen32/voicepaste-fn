import Foundation

/// Abstracts any speech-to-text backend (server or local).
public protocol TranscriptionService {
    /// Transcribe an audio file.
    /// - Parameters:
    ///   - fileURL: Local path to the audio file.
    ///   - languageCode: BCP-47 code (e.g. "ru", "en") or nil for auto-detect.
    /// - Returns: The transcribed text.
    func transcribe(fileURL: URL, languageCode: String?) throws -> String
}

/// Orchestrates server transcription with automatic retries and optional local fallback.
public final class RetryTranscriber {
    private let primary: TranscriptionService
    private let fallback: TranscriptionService?
    private let maxAttempts: Int

    public init(primary: TranscriptionService, fallback: TranscriptionService?, maxAttempts: Int = 3) {
        self.primary = primary
        self.fallback = fallback
        self.maxAttempts = max(1, maxAttempts)
    }

    /// Attempt transcription up to `maxAttempts` times with the primary service.
    /// If all attempts fail and a fallback is configured, try the fallback once.
    /// Throws the last primary error if everything fails.
    public func transcribe(fileURL: URL, languageCode: String?) throws -> String {
        var lastError: Error?

        for attempt in 1...maxAttempts {
            do {
                let text = try primary.transcribe(fileURL: fileURL, languageCode: languageCode)
                return text
            } catch {
                lastError = error
                print("transcription attempt \(attempt)/\(maxAttempts) failed: \(error.localizedDescription)")
            }
        }

        // All primary attempts failed — try fallback
        if let fallback = fallback {
            do {
                let text = try fallback.transcribe(fileURL: fileURL, languageCode: languageCode)
                return text
            } catch {
                print("fallback transcription failed: \(error.localizedDescription)")
                // Throw the original primary error for consistency
            }
        }

        throw lastError ?? NSError(domain: "VoicePaste", code: 99,
                                   userInfo: [NSLocalizedDescriptionKey: "All transcription attempts failed"])
    }
}
