import Foundation
import NativeSTT
import Speech

// MARK: - JSON line types

/// One JSON line on stdout per invocation. The Rust parent reads these
/// and parses them with `serde_json::from_str`.
struct ResultPayload: Codable {
    let type: String
    let text: String
    let locale: String
}

struct ErrorPayload: Codable {
    let type: String
    let message: String
    let code: String
}

// MARK: - I/O helpers

/// Write a Codable payload as a single JSON line on the given file handle.
/// We always append a trailing newline so the Rust parent's `read_line`
/// sees a complete line.
func writeJSON<T: Encodable>(_ payload: T, to handle: FileHandle) {
    let encoder = JSONEncoder()
    encoder.outputFormatting = []
    do {
        var data = try encoder.encode(payload)
        data.append(0x0A) // '\n'
        handle.write(data)
    } catch {
        // Last-ditch: write a raw error line. We can't recurse (would loop)
        // if the encoder itself is broken, so just dump the description.
        let fallback = "{\"type\":\"error\",\"message\":\"JSON encode failed: \(error)\",\"code\":\"encode_failed\"}\n"
        if let bytes = fallback.data(using: .utf8) {
            handle.write(bytes)
        }
    }
}

/// Map a `NativeSTTService.RecognitionError` case to a stable error code
/// the Rust parent can pattern-match on.
func errorCode(for error: NativeSTTService.RecognitionError) -> String {
    switch error {
    case .authorizationDenied:           return "auth_denied"
    case .authorizationRestricted:       return "auth_restricted"
    case .recognizerUnavailable:         return "recognizer_unavailable"
    case .fileNotFound:                  return "file_not_found"
    case .fileReadFailed:                return "file_read_failed"
    case .audioConversionFailed:         return "audio_conversion_failed"
    case .recognitionFailed:             return "recognition_failed"
    case .timeout:                       return "timeout"
    }
}

// MARK: - CLI

func usage() -> Never {
    let stderr = FileHandle.standardError
    stderr.write(Data("Usage: native_stt <wav_path> <locale>\n".utf8))
    stderr.write(Data("  wav_path: absolute path to a WAV file\n".utf8))
    stderr.write(Data("  locale:   BCP-47 code (e.g. ru, ru-RU, en-US) or 'auto'\n".utf8))
    exit(1)
}

let args = CommandLine.arguments
if args.count == 2, args[1] == "--permissions" {
    let status = SFSpeechRecognizer.authorizationStatus()
    let payload: [String: Any] = [
        "speech_recognition": status == .authorized,
        "status": String(describing: status),
    ]
    let data = try! JSONSerialization.data(withJSONObject: payload, options: [.sortedKeys])
    FileHandle.standardOutput.write(data)
    FileHandle.standardOutput.write(Data("\n".utf8))
    exit(status == .authorized ? 0 : 1)
}
guard args.count == 3 else { usage() }
let wavPath = args[1]
let locale = args[2]

// Run an async entry point on a dedicated dispatch group. We block on a
// semaphore so the process exits when recognition completes.
let semaphore = DispatchSemaphore(value: 0)
let service = NativeSTTService()
let stdout = FileHandle.standardOutput
let stderr = FileHandle.standardError

Task.detached {
    do {
        let text = try await service.recognize(wavPath: wavPath, locale: locale)
        let payload = ResultPayload(type: "result", text: text, locale: locale)
        writeJSON(payload, to: stdout)
        exit(0)
    } catch let err as NativeSTTService.RecognitionError {
        let payload = ErrorPayload(
            type: "error",
            message: err.errorDescription ?? "unknown error",
            code: errorCode(for: err)
        )
        writeJSON(payload, to: stderr)
        exit(1)
    } catch {
        let payload = ErrorPayload(
            type: "error",
            message: error.localizedDescription,
            code: "internal_error"
        )
        writeJSON(payload, to: stderr)
        exit(1)
    }
}

// Park the main thread until recognition completes. We use a very long
// timeout — the helper itself enforces a 30s recognition timeout, so this
// semaphore either unblocks (success) or we hang and the parent kills us
// (timeout). That's intentional: it gives Speech time to make progress
// without the parent having to babysit.
semaphore.wait()
