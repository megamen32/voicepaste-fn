import AppKit
import AVFoundation
import ApplicationServices
import Foundation
import Security
import VoicePasteLib

// MARK: - Audio Recorder

final class Recorder: NSObject, AVAudioRecorderDelegate {
    private var recorder: AVAudioRecorder?
    private(set) var currentURL: URL?

    var currentTime: TimeInterval {
        recorder?.currentTime ?? 0
    }

    func start() throws {
        stopWithoutReturning()

        let dir = FileManager.default.temporaryDirectory.appendingPathComponent("voicepaste-fn", isDirectory: true)
        try FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
        let url = dir.appendingPathComponent(UUID().uuidString).appendingPathExtension("wav")

        let settings: [String: Any] = [
            AVFormatIDKey: Int(kAudioFormatLinearPCM),
            AVSampleRateKey: 16000,
            AVNumberOfChannelsKey: 1,
            AVLinearPCMBitDepthKey: 16,
            AVLinearPCMIsFloatKey: false,
            AVLinearPCMIsBigEndianKey: false
        ]

        let rec = try AVAudioRecorder(url: url, settings: settings)
        rec.delegate = self
        rec.isMeteringEnabled = true
        rec.prepareToRecord()
        guard rec.record() else {
            throw NSError(domain: "VoicePaste", code: 10, userInfo: [NSLocalizedDescriptionKey: "AVAudioRecorder.record() returned false"])
        }

        recorder = rec
        currentURL = url
    }

    func stop() -> URL? {
        recorder?.stop()
        recorder = nil
        let url = currentURL
        currentURL = nil
        return url
    }

    func stopWithoutReturning() {
        recorder?.stop()
        recorder = nil
        if let url = currentURL {
            try? FileManager.default.removeItem(at: url)
        }
        currentURL = nil
    }

    func averagePower() -> Float {
        recorder?.updateMeters()
        return recorder?.averagePower(forChannel: 0) ?? -160
    }
}

// MARK: - Transcription

final class Transcriber {
    // No stored config — reads SettingsStore on every request so the user
    // can edit endpoint/API key in the menu bar and the next transcription
    // picks up the new values without a restart.

    func transcribe(fileURL: URL, language: Language, model: String? = nil) throws -> String {
        let store = SettingsStore.shared
        guard !store.baseURL.isEmpty,
              let baseURL = URL(string: store.baseURL.trimmingCharacters(in: CharacterSet(charactersIn: "/"))) else {
            throw NSError(domain: "VoicePaste", code: 30, userInfo: [
                NSLocalizedDescriptionKey: "Endpoint URL is invalid. Open VoicePaste Fn menu → Endpoint → Edit…"
            ])
        }
        var request = URLRequest(url: baseURL.appendingPathComponent("audio/transcriptions"))
        request.httpMethod = "POST"
        if !store.apiKey.isEmpty {
            request.setValue("Bearer \(store.apiKey)", forHTTPHeaderField: "Authorization")
        }

        let boundary = "Boundary-\(UUID().uuidString)"
        request.setValue("multipart/form-data; boundary=\(boundary)", forHTTPHeaderField: "Content-Type")

        var body = Data()
        if let model = model, !model.isEmpty {
            appendField(name: "model", value: model, boundary: boundary, body: &body)
        }
        appendField(name: "response_format", value: "json", boundary: boundary, body: &body)
        if let languageValue = language.apiValue {
            appendField(name: "language", value: languageValue, boundary: boundary, body: &body)
        }
        try appendFile(name: "file", filename: "audio.wav", mime: "audio/wav", url: fileURL, boundary: boundary, body: &body)
        body.appendString("--\(boundary)--\r\n")
        request.httpBody = body

        let sem = DispatchSemaphore(value: 0)
        var resultData: Data?
        var resultResponse: URLResponse?
        var resultError: Error?

        URLSession.shared.dataTask(with: request) { data, response, error in
            resultData = data
            resultResponse = response
            resultError = error
            sem.signal()
        }.resume()

        sem.wait()

        if let error = resultError { throw error }
        guard let http = resultResponse as? HTTPURLResponse else {
            throw NSError(domain: "VoicePaste", code: 1, userInfo: [NSLocalizedDescriptionKey: "No HTTP response"])
        }

        let data = resultData ?? Data()
        guard (200..<300).contains(http.statusCode) else {
            let message = String(data: data, encoding: .utf8) ?? "HTTP \(http.statusCode)"
            throw NSError(domain: "VoicePaste", code: http.statusCode, userInfo: [NSLocalizedDescriptionKey: message])
        }

        let json = try JSONSerialization.jsonObject(with: data) as? [String: Any]
        let rawText = (json?["text"] as? String)?.trimmingCharacters(in: .whitespacesAndNewlines) ?? ""
        return TextCleaner.clean(rawText)
    }

    private func appendField(name: String, value: String, boundary: String, body: inout Data) {
        body.appendString("--\(boundary)\r\n")
        body.appendString("Content-Disposition: form-data; name=\"\(name)\"\r\n\r\n")
        body.appendString("\(value)\r\n")
    }

    private func appendFile(name: String, filename: String, mime: String, url: URL, boundary: String, body: inout Data) throws {
        body.appendString("--\(boundary)\r\n")
        body.appendString("Content-Disposition: form-data; name=\"\(name)\"; filename=\"\(filename)\"\r\n")
        body.appendString("Content-Type: \(mime)\r\n\r\n")
        body.append(try Data(contentsOf: url))
        body.appendString("\r\n")
    }

    func fetchModels() -> [String] {
        let store = SettingsStore.shared
        guard let client = try? ModelListClient(baseURL: store.baseURL, apiKey: store.apiKey) else {
            return []
        }
        return (try? client.fetchModels()) ?? []
    }
}

/// Adapts the existing `Transcriber` to the `TranscriptionService` protocol
/// so it can be used as the primary service in `RetryTranscriber`.
final class ServerTranscriptionService: TranscriptionService {
    private let transcriber: Transcriber
    private let language: Language
    private let model: String?

    init(transcriber: Transcriber, language: Language, model: String?) {
        self.transcriber = transcriber
        self.language = language
        self.model = model
    }

    func transcribe(fileURL: URL, languageCode: String?) throws -> String {
        // Use the provided languageCode if available, otherwise fall back to configured language
        let lang = Language(rawValue: languageCode ?? "") ?? language
        return try transcriber.transcribe(fileURL: fileURL, language: lang, model: model)
    }
}

extension Data {
    mutating func appendString(_ string: String) {
        append(string.data(using: .utf8)!)
    }
}

// MARK: - Text Cleanup
//
// Strip common subtitle-channel boilerplate that YouTube transcripts
// often leave at the end ("Продолжение следует", "Thanks for watching!",
// "Subtitles by DimaTorzok"). Match is case-insensitive, only fires when
// the phrase is at the end of the text, and tolerates an optional trailing
// punctuation cluster (one or more of .,!?,* — and any mix of dots).

final class TextCleaner {
    private static let unwantedSuffixes = [
        // Russian: "продолжение следует" with optional trailing dots/sparkle
        "продолжение следует",
        // Russian subtitle credit
        "субтитры сделал DimaTorzok",
        "субтитры сделаны DimaTorzok",
        // English subtitle credit
        "subtitles by DimaTorzok",
        "subtitles made by DimaTorzok",
        // English to-be-continued
        "to be continued",
        // English outro
        "thanks for watching",
    ]

    /// Punctuation characters that may sit at the very end of the text but
    /// should not stop us from matching a suffix.
    private static let trailingPunct: Set<Character> = [".", "!", "?", "*", ";", ":"]

    static func clean(_ text: String) -> String {
        var result = text.trimmingCharacters(in: .whitespacesAndNewlines)
        if result.isEmpty { return result }

        let lowerResult = result.lowercased()

        for suffix in unwantedSuffixes {
            let lowerSuffix = suffix.lowercased()
            guard !lowerSuffix.isEmpty else { continue }

            // Try the bare suffix first.
            var matched = lowerResult.hasSuffix(lowerSuffix)
            // Then try with 1–4 trailing punctuation chars (e.g. "!?", "...", "!",
            // ".", "***", "....", "!?!" — anything an over-eager transcriber
            // appends after the actual phrase).
            if !matched {
                var probe = String(result.suffix(8)).lowercased()
                for _ in 0..<5 {
                    // Did the trailing chars all become punctuation?
                    if probe.hasSuffix(lowerSuffix) {
                        matched = true
                        break
                    }
                    guard let last = probe.last, trailingPunct.contains(last) else { break }
                    probe.removeLast()
                }
            }
            guard matched else { continue }

            // Find the cut point in the ORIGINAL-CASE result.
            // Walk backwards over any trailing punctuation, then over the
            // suffix characters themselves.
            var cutIndex = result.endIndex
            // Strip trailing punctuation (one or more chars from the set).
            while cutIndex > result.startIndex {
                let prevIndex = result.index(before: cutIndex)
                let lastChar = result[prevIndex]
                if trailingPunct.contains(lastChar) {
                    cutIndex = prevIndex
                } else {
                    break
                }
            }
            // Strip the suffix itself.
            let target = result.index(cutIndex, offsetBy: -suffix.count)
            if target >= result.startIndex {
                cutIndex = target
            } else {
                cutIndex = result.startIndex
            }

            result = String(result[..<cutIndex])
                .trimmingCharacters(in: .whitespacesAndNewlines)
        }
        return result
    }
}

// MARK: - Clipboard Paste

final class PasteboardTyper {
    func paste(_ text: String, targetPID: pid_t? = nil) {
        let trimmed = text.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty else { return }

        let pb = NSPasteboard.general
        pb.clearContents()
        pb.setString(trimmed, forType: .string)

        if let targetPID, let target = NSRunningApplication(processIdentifier: targetPID) {
            target.activate(options: [.activateIgnoringOtherApps])
        }

        usleep(80_000)

        let source = CGEventSource(stateID: .combinedSessionState)
        let keyDown = CGEvent(keyboardEventSource: source, virtualKey: 0x09, keyDown: true)   // V
        let keyUp = CGEvent(keyboardEventSource: source, virtualKey: 0x09, keyDown: false)
        keyDown?.flags = .maskCommand
        keyUp?.flags = .maskCommand
        if let targetPID {
            keyDown?.postToPid(targetPID)
            keyUp?.postToPid(targetPID)
        } else {
            keyDown?.post(tap: .cghidEventTap)
            keyUp?.post(tap: .cghidEventTap)
        }
    }
}

// MARK: - Autostart

final class AutostartManager {
    static let label = "com.bezrabotnyi.voicepastefn"

    static var plistURL: URL {
        FileManager.default.homeDirectoryForCurrentUser
            .appendingPathComponent("Library/LaunchAgents", isDirectory: true)
            .appendingPathComponent("\(label).plist")
    }

    static func setEnabled(_ enabled: Bool) {
        let fm = FileManager.default
        let launchAgents = plistURL.deletingLastPathComponent()
        try? fm.createDirectory(at: launchAgents, withIntermediateDirectories: true)

        // Resolve project root: walk up from executable to find run.sh
        // (handles both direct execution and .app bundle where binary is deep inside)
        let execURL: URL
        if let bundlePath = Bundle.main.executableURL {
            execURL = bundlePath
        } else {
            execURL = URL(fileURLWithPath: CommandLine.arguments[0])
        }
        var searchDir = execURL.deletingLastPathComponent()
        let fm2 = FileManager.default
        var projectRoot = searchDir
        for _ in 0..<10 {
            if fm2.fileExists(atPath: searchDir.appendingPathComponent("run.sh").path) {
                projectRoot = searchDir
                break
            }
            let parent = searchDir.deletingLastPathComponent()
            if parent == searchDir { break }
            searchDir = parent
        }
        let cwd = projectRoot
        let run = cwd.appendingPathComponent("run.sh").path

        if enabled {
            let plist = """
            <?xml version="1.0" encoding="UTF-8"?>
            <!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
            <plist version="1.0">
            <dict>
              <key>Label</key><string>\(label)</string>
              <key>ProgramArguments</key>
              <array>
                <string>/bin/bash</string>
                <string>\(run)</string>
              </array>
              <key>WorkingDirectory</key><string>\(cwd.path)</string>
              <key>RunAtLoad</key><true/>
              <key>KeepAlive</key><false/>
              <key>StandardOutPath</key><string>/tmp/voicepaste-fn.out.log</string>
              <key>StandardErrorPath</key><string>/tmp/voicepaste-fn.err.log</string>
            </dict>
            </plist>
            """
            try? plist.write(to: plistURL, atomically: true, encoding: .utf8)
            _ = shell("launchctl unload \(plistURL.path.shellEscaped()) 2>/dev/null || true")
            _ = shell("launchctl load \(plistURL.path.shellEscaped())")
        } else {
            _ = shell("launchctl unload \(plistURL.path.shellEscaped()) 2>/dev/null || true")
            try? fm.removeItem(at: plistURL)
        }
    }

    private static func shell(_ command: String) -> Int32 {
        let p = Process()
        p.launchPath = "/bin/bash"
        p.arguments = ["-lc", command]
        try? p.run()
        p.waitUntilExit()
        return p.terminationStatus
    }
}

extension String {
    func shellEscaped() -> String {
        "'" + self.replacingOccurrences(of: "'", with: "'\\''") + "'"
    }
}

// MARK: - Hotkey

/// Which physical key triggers dictation. Stored in UserDefaults so it
/// survives relaunch. `keyCode` is CGKeyCode (only for non-modifier keys);
/// `flag` is the corresponding CGEventFlags bit (only for modifier keys).
/// Modifier keys normally use flagsChanged; Fn/Globe and Caps Lock also have
/// keycode paths on keyboards that report them as ordinary key events.
enum HotkeyKind: String, CaseIterable {
    case fn                 // .maskSecondaryFn (Globe / Fn on Apple Magic)
    case rightOption        // flag .maskAlternate, right-side (location-based)
    case rightControl       // flag .maskControl,   right-side
    case rightCommand       // flag .maskCommand,   right-side
    case rightShift         // flag .maskShift,     right-side
    case capsLock           // keyCode 57
    case f13                // keyCode 105
    case f14                // keyCode 107
    case f15                // keyCode 113

    var title: String {
        switch self {
        case .fn:           return "Fn (Globe)"
        case .rightOption:  return "Right ⌥ Option"
        case .rightControl: return "Right ⌃ Control"
        case .rightCommand: return "Right ⌘ Command"
        case .rightShift:   return "Right ⇧ Shift"
        case .capsLock:     return "Caps Lock"
        case .f13:          return "F13"
        case .f14:          return "F14"
        case .f15:          return "F15"
        }
    }

    /// Fn/Globe is reported as a key event on some Apple Silicon keyboards
    /// and as flagsChanged on others. Listen to both paths so release cannot
    /// leave recording stuck in the active state.
    var targetKeyCodes: Set<CGKeyCode> {
        switch self {
        case .fn: return [63, 179]
        case .capsLock: return [57]
        default: return []
        }
    }

    var flag: CGEventFlags? {
        switch self {
        case .fn:           return .maskSecondaryFn
        case .rightOption:  return .maskAlternate
        case .rightControl: return .maskControl
        case .rightCommand: return .maskCommand
        case .rightShift:   return .maskShift
        default:            return nil
        }
    }

    /// For modifier-based hotkeys we require the event to come from the
    /// right-side of the keyboard (so Left ⌥ / Left ⌘ etc. still behave
    /// normally for shortcuts the user already uses). We use the keyboard
    /// input events tap's `event_unix_flags` field which encodes the
    /// hardware location bit.
    var requiresRightSide: Bool {
        switch self {
        case .rightOption, .rightControl, .rightCommand, .rightShift:
            return true
        default:
            return false
        }
    }
}

enum ActivationMode: String, CaseIterable {
    case hold
    case toggle

    var title: String {
        switch self {
        case .hold:   return "Hold (press to start, release to stop)"
        case .toggle: return "Toggle (press to start, press again to stop)"
        }
    }
}

// MARK: - Wake-up silence WAV

/// Builds and caches a 1-second 16-kHz mono 16-bit PCM silence WAV used
/// as a warm-up request payload. The cache lives in `NSTemporaryDirectory()`
/// (cleaned by macOS), so we don't litter `~/Library/...` if the app
/// crashes mid-write. The first call writes ~32 KB; subsequent calls return
/// the same file.
final class WakeWav {
    static let shared = WakeWav()
    private init() {}

    private let sampleRate: UInt32 = 16_000
    private let duration: Double = 1.0
    private var cached: URL?

    func ensureSilenceWav() throws -> URL {
        if let cached, FileManager.default.fileExists(atPath: cached.path) {
            return cached
        }
        let dir = FileManager.default.temporaryDirectory
            .appendingPathComponent("voicepaste-fn-wake", isDirectory: true)
        try FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
        let url = dir.appendingPathComponent("silence-1s.wav")

        let numSamples = Int(Double(sampleRate) * duration)
        let dataSize = UInt32(numSamples * 2)            // mono × 16-bit
        let fileSize = UInt32(36 + dataSize)             // 36 = header size

        var d = Data()
        d.append(ascii: "RIFF")
        d.appendLE(UInt32: fileSize)
        d.append(ascii: "WAVE")
        d.append(ascii: "fmt ")
        d.appendLE(UInt32: 16)                           // PCM fmt chunk size
        d.appendLE(UInt16: 1)                            // PCM format
        d.appendLE(UInt16: 1)                            // channels (mono)
        d.appendLE(UInt32: sampleRate)
        d.appendLE(UInt32: sampleRate * 2)               // byte rate
        d.appendLE(UInt16: 2)                            // block align
        d.appendLE(UInt16: 16)                           // bits per sample
        d.append(ascii: "data")
        d.appendLE(UInt32: dataSize)
        d.append(Data(repeating: 0, count: Int(dataSize)))   // 1 s of silence

        try d.write(to: url, options: .atomic)
        cached = url
        return url
    }
}

private extension Data {
    mutating func append(ascii s: String) {
        if let b = s.data(using: .ascii) { append(b) }
    }
    mutating func appendLE(UInt32 value: UInt32) {
        var v = value.littleEndian
        Swift.withUnsafeBytes(of: &v) { append(contentsOf: $0) }
    }
    mutating func appendLE(UInt16 value: UInt16) {
        var v = value.littleEndian
        Swift.withUnsafeBytes(of: &v) { append(contentsOf: $0) }
    }
}

// MARK: - App

let app = NSApplication.shared
let delegate = VoicePasteApp()
app.delegate = delegate
app.setActivationPolicy(.accessory)
app.run()
