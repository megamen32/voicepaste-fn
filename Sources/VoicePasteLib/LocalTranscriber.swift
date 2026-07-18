import Foundation
import Speech
import AVFoundation

/// Local on-device transcription using Apple's Speech framework.
/// Conforms to `TranscriptionService` so it can be used as a fallback.
public final class LocalTranscriber: TranscriptionService {

    public init() {}

    public func transcribe(fileURL: URL, languageCode: String?) throws -> String {
        // Check authorization
        let status = SFSpeechRecognizer.authorizationStatus()
        guard status == .authorized else {
            throw LocalTranscriptionError.notAuthorized
        }

        // Pick locale-based recognizer if languageCode is nil
        let locale: Locale
        if let code = languageCode, !code.isEmpty {
            locale = Locale(identifier: code)
        } else {
            locale = Locale.current
        }

        guard let recognizer = SFSpeechRecognizer(locale: locale), recognizer.isAvailable else {
            throw LocalTranscriptionError.recognizerUnavailable
        }

        let request = SFSpeechAudioBufferRecognitionRequest()
        request.shouldReportPartialResults = false
        // Prefer on-device if available
        if recognizer.supportsOnDeviceRecognition {
            request.requiresOnDeviceRecognition = true
        }

        // Open audio file and feed PCM buffers to the recognizer
        guard let audioFile = try? AVAudioFile(forReading: fileURL) else {
            throw LocalTranscriptionError.cannotReadFile
        }

        let semaphore = DispatchSemaphore(value: 0)
        var resultText: String?
        var resultError: Error?

        // Read the entire file into a buffer and append to request
        let format = audioFile.processingFormat
        guard let buffer = AVAudioPCMBuffer(pcmFormat: format, frameCapacity: AVAudioFrameCount(audioFile.length)) else {
            throw LocalTranscriptionError.cannotReadFile
        }
        buffer.frameLength = AVAudioFrameCount(audioFile.length)

        do {
            try audioFile.read(into: buffer)
        } catch {
            throw LocalTranscriptionError.cannotReadFile
        }

        request.append(buffer)
        request.endAudio()

        recognizer.recognitionTask(with: request) { result, error in
            if let result = result {
                resultText = result.bestTranscription.formattedString
            }
            if error != nil || result != nil {
                resultError = error
                semaphore.signal()
            }
        }

        let timeout = semaphore.wait(timeout: .now() + 30)
        if timeout == .timedOut {
            throw LocalTranscriptionError.recognitionFailed("Recognition timed out after 30 seconds")
        }

        if let error = resultError {
            throw LocalTranscriptionError.recognitionFailed(error.localizedDescription)
        }

        guard let text = resultText?.trimmingCharacters(in: .whitespacesAndNewlines), !text.isEmpty else {
            throw LocalTranscriptionError.emptyResult
        }

        return text
    }
}

public enum LocalTranscriptionError: Error, LocalizedError {
    case notAuthorized
    case recognizerUnavailable
    case cannotReadFile
    case recognitionFailed(String)
    case emptyResult

    public var errorDescription: String? {
        switch self {
        case .notAuthorized:
            return "Speech recognition is not authorized. Grant permission in System Settings."
        case .recognizerUnavailable:
            return "Local speech recognizer is not available for this language."
        case .cannotReadFile:
            return "Cannot read the audio file for local transcription."
        case .recognitionFailed(let reason):
            return "Local transcription failed: \(reason)"
        case .emptyResult:
            return "Local transcription returned empty result."
        }
    }
}
