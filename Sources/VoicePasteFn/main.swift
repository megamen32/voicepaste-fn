import AppKit
import AVFoundation
import ApplicationServices
import Foundation
import Security
import VoicePasteLib

// MARK: - Settings (UserDefaults + Keychain, hot-reloadable)
//
// Storage layout follows macOS conventions:
//   - non-secret settings → UserDefaults
//     (persisted to ~/Library/Preferences/com.bezrabotnyi.voicepastefn.plist)
//   - API key → Keychain (system-encrypted, only this app can read)
//   - env vars override UserDefaults/Keychain for the current launch
//     (lets a shell-launched `swift run` override what's saved).
//
// Reads happen lazily on every access, so updating a value in the menu
// bar and saving takes effect on the very next transcription — no
// restart required.

private let kKeychainService = "com.bezrabotnyi.voicepastefn"
private let kKeychainAccountAPIKey = "openai_api_key"

private let kDefaultsKeyBaseURL = "openai_base_url"
private let kDefaultsKeyModel = "transcribe_model"
private let kDefaultsKeyBaseURLSet = "openai_base_url_set"   // distinguishes "unset" from "= ''"

/// Default endpoint shown in the "Edit…" dialog the very first time.
private let kDefaultBaseURL = "https://api.openai.com/v1"
private let kDefaultModel = "whisper-1"

final class SettingsStore {
    static let shared = SettingsStore()
    private init() {}

    private let defaults = UserDefaults.standard

    // MARK: Base URL
    var baseURL: String {
        // Env override wins.
        if let env = ProcessInfo.processInfo.environment["OPENAI_BASE_URL"], !env.isEmpty {
            return env
        }
        if defaults.bool(forKey: kDefaultsKeyBaseURLSet),
           let saved = defaults.string(forKey: kDefaultsKeyBaseURL),
           !saved.isEmpty {
            return saved
        }
        return kDefaultBaseURL
    }

    func setBaseURL(_ value: String) throws {
        let trimmed = value.trimmingCharacters(in: CharacterSet(charactersIn: "/"))
        guard !trimmed.isEmpty, URL(string: trimmed) != nil else {
            throw NSError(domain: "VoicePaste", code: 20,
                          userInfo: [NSLocalizedDescriptionKey: "Invalid URL: \(value)"])
        }
        defaults.set(trimmed, forKey: kDefaultsKeyBaseURL)
        defaults.set(true, forKey: kDefaultsKeyBaseURLSet)
    }

    // MARK: API key (Keychain)
    var apiKey: String {
        if let env = ProcessInfo.processInfo.environment["OPENAI_API_KEY"], !env.isEmpty {
            return env
        }
        return readKeychainAPIKey() ?? ""
    }

    func setAPIKey(_ value: String) throws {
        try writeKeychainAPIKey(value)
    }

    func clearAPIKey() {
        deleteKeychainAPIKey()
    }

    // MARK: Model (Whisper)
    var model: String {
        if let env = ProcessInfo.processInfo.environment["TRANSCRIBE_MODEL"], !env.isEmpty {
            return env
        }
        return defaults.string(forKey: kDefaultsKeyModel) ?? kDefaultModel
    }

    func setModel(_ value: String) {
        defaults.set(value, forKey: kDefaultsKeyModel)
    }

    // MARK: Display helpers
    var maskedBaseURL: String {
        // Show the host + first path segment so the user can tell which
        // endpoint they're talking to without exposing the full URL.
        let u = URL(string: baseURL)
        if let host = u?.host {
            return host
        }
        return baseURL
    }

    var maskedAPIKey: String {
        let k = apiKey
        guard !k.isEmpty else { return "(not set)" }
        if k.count <= 8 {
            return String(repeating: "•", count: k.count)
        }
        let prefix = k.prefix(3)
        let suffix = k.suffix(4)
        return "\(prefix)•••\(suffix)  (\(k.count) chars)"
    }

    var isConfigured: Bool {
        !baseURL.isEmpty && !apiKey.isEmpty
    }

    // MARK: - Keychain helpers (kSecClassGenericPassword)
    private func readKeychainAPIKey() -> String? {
        var query: [String: Any] = [
            kSecClass as String: kSecClassGenericPassword,
            kSecAttrService as String: kKeychainService,
            kSecAttrAccount as String: kKeychainAccountAPIKey,
            kSecReturnData as String: true,
            kSecMatchLimit as String: kSecMatchLimitOne,
        ]
        var result: AnyObject?
        let status = SecItemCopyMatching(query as CFDictionary, &result)
        guard status == errSecSuccess, let data = result as? Data else {
            return nil
        }
        return String(data: data, encoding: .utf8)
    }

    private func writeKeychainAPIKey(_ value: String) throws {
        // Delete any existing item first — SecItemUpdate can be finicky about
        // the data attribute on a fresh keychain.
        let baseQuery: [String: Any] = [
            kSecClass as String: kSecClassGenericPassword,
            kSecAttrService as String: kKeychainService,
            kSecAttrAccount as String: kKeychainAccountAPIKey,
        ]
        SecItemDelete(baseQuery as CFDictionary)

        if value.isEmpty {
            return
        }
        var addQuery = baseQuery
        addQuery[kSecValueData as String] = value.data(using: .utf8) ?? Data()
        addQuery[kSecAttrAccessible as String] = kSecAttrAccessibleAfterFirstUnlock
        let status = SecItemAdd(addQuery as CFDictionary, nil)
        guard status == errSecSuccess else {
            throw NSError(
                domain: "VoicePaste", code: Int(status),
                userInfo: [NSLocalizedDescriptionKey:
                    "Keychain write failed (status \(status)). " +
                    "If the system keeps prompting for permission, allow VoicePasteFn " +
                    "in Keychain Access (System Settings → Privacy & Security)."]
            )
        }
    }

    private func deleteKeychainAPIKey() {
        let query: [String: Any] = [
            kSecClass as String: kSecClassGenericPassword,
            kSecAttrService as String: kKeychainService,
            kSecAttrAccount as String: kKeychainAccountAPIKey,
        ]
        SecItemDelete(query as CFDictionary)
    }
}

enum Language: String, CaseIterable {
    case ru
    case en
    case auto

    var title: String {
        switch self {
        case .ru: return "Russian / ru"
        case .en: return "English / en"
        case .auto: return "Auto"
        }
    }

    var apiValue: String? {
        switch self {
        case .ru: return "ru"
        case .en: return "en"
        case .auto: return nil
        }
    }
}

final class Settings {
    static let shared = Settings()

    private let defaults = UserDefaults.standard

    private enum Key {
        static let language = "language"
        static let realtimePreview = "realtimePreview"
        static let autostart = "autostart"
        static let selectedModel = "selectedModel"
        static let recordingDelay = "recordingDelay"
        static let hideDelay = "hideDelay"
        static let hotkey = "hotkey"
        static let activationMode = "activationMode"
        static let overlayCentered = "overlayCentered"
        static let wakeServerOnStart = "wakeServerOnStart"
        static let realtimeChunkInterval = "realtimeChunkInterval"
        static let appleFallback = "appleFallback"
    }

    private init() {
        if defaults.string(forKey: Key.language) == nil {
            defaults.set(Language.ru.rawValue, forKey: Key.language)
        }
    }

    var language: Language {
        get { Language(rawValue: defaults.string(forKey: Key.language) ?? "ru") ?? .ru }
        set { defaults.set(newValue.rawValue, forKey: Key.language) }
    }

    var realtimePreview: Bool {
        get { defaults.bool(forKey: Key.realtimePreview) }
        set { defaults.set(newValue, forKey: Key.realtimePreview) }
    }

    var autostart: Bool {
        get { defaults.bool(forKey: Key.autostart) }
        set { defaults.set(newValue, forKey: Key.autostart) }
    }

    var selectedModel: String {
        get { defaults.string(forKey: Key.selectedModel) ?? "auto" }
        set { defaults.set(newValue, forKey: Key.selectedModel) }
    }

    /// How long Fn must be held before recording actually starts (debounce).
    /// Range 0.10–2.00 seconds; default 0.20. Clamped on read so a corrupted
    /// or out-of-range value from disk never breaks the recorder.
    var recordingDelay: TimeInterval {
        get {
            let stored = defaults.double(forKey: Key.recordingDelay)
            // 0 means never set; treat as default.
            let raw = stored == 0 ? 0.20 : stored
            return min(2.0, max(0.1, raw))
        }
        set {
            let clamped = min(2.0, max(0.1, newValue))
            defaults.set(clamped, forKey: Key.recordingDelay)
        }
    }

    /// How long the overlay stays visible after the final transcript is pasted,
    /// so the user can read what got written to the clipboard.
    /// Range 0.0 – 5.0 seconds; default 0.8 (0 means "never hide automatically",
    /// user dismisses by holding Fn again or by clicking).
    var hideDelay: TimeInterval {
        get {
            let stored = defaults.double(forKey: Key.hideDelay)
            let raw = stored == 0 ? 0.8 : stored
            return min(5.0, max(0.0, raw))
        }
        set {
            let clamped = min(5.0, max(0.0, newValue))
            defaults.set(clamped, forKey: Key.hideDelay)
        }
    }

    /// Which hotkey triggers recording. Default: Fn.
    /// Stored as the rawValue string of HotkeyKind (see HotkeyKind enum).
    var hotkey: HotkeyKind {
        get {
            let raw = defaults.string(forKey: Key.hotkey) ?? HotkeyKind.fn.rawValue
            return HotkeyKind(rawValue: raw) ?? .fn
        }
        set { defaults.set(newValue.rawValue, forKey: Key.hotkey) }
    }

    /// How the hotkey triggers: hold (press to start, release to stop) or
    /// toggle (first press starts, second press stops). Default: hold.
    var activationMode: ActivationMode {
        get {
            let raw = defaults.string(forKey: Key.activationMode) ?? ActivationMode.hold.rawValue
            return ActivationMode(rawValue: raw) ?? .hold
        }
        set { defaults.set(newValue.rawValue, forKey: Key.activationMode) }
    }

    /// Whether to put the preview overlay at the cursor's position
    /// (default-ish, "follow mouse") or centred on screen.
    var overlayCentered: Bool {
        get { defaults.bool(forKey: Key.overlayCentered) }
        set { defaults.set(newValue, forKey: Key.overlayCentered) }
    }

    /// Ping the Whisper endpoint at dictation start so the server-side
    /// model isn't unloaded by idle-timeout between sessions. Default: true.
    var wakeServerOnStart: Bool {
        get {
            if defaults.object(forKey: Key.wakeServerOnStart) == nil { return true }
            return defaults.bool(forKey: Key.wakeServerOnStart)
        }
        set { defaults.set(newValue, forKey: Key.wakeServerOnStart) }
    }

    /// Realtime preview: how often (in seconds) to transcribe a chunk while
    /// the user is still holding the hotkey. Range 1.0 – 30.0 s; default 5.0.
    /// Smaller values feel more responsive but hit the API harder.
    var realtimeChunkInterval: TimeInterval {
        get {
            let stored = defaults.double(forKey: Key.realtimeChunkInterval)
            let raw = stored == 0 ? 5.0 : stored
            return min(30.0, max(1.0, raw))
        }
        set {
            let clamped = min(30.0, max(1.0, newValue))
            defaults.set(clamped, forKey: Key.realtimeChunkInterval)
        }
    }

    /// When enabled, if the Whisper server fails 3 times, fall back to
    /// Apple's on-device speech recognition for the same audio.
    var appleFallback: Bool {
        get { defaults.bool(forKey: Key.appleFallback) }
        set { defaults.set(newValue, forKey: Key.appleFallback) }
    }
}

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
        guard !store.baseURL.isEmpty,
              let baseURL = URL(string: store.baseURL.trimmingCharacters(in: CharacterSet(charactersIn: "/"))) else {
            return []
        }
        var request = URLRequest(url: baseURL.appendingPathComponent("models"))
        request.httpMethod = "GET"
        if !store.apiKey.isEmpty {
            request.setValue("Bearer \(store.apiKey)", forHTTPHeaderField: "Authorization")
        }
        request.timeoutInterval = 10

        let sem = DispatchSemaphore(value: 0)
        var resultData: Data?
        URLSession.shared.dataTask(with: request) { data, _, _ in
            resultData = data
            sem.signal()
        }.resume()
        sem.wait()

        guard let data = resultData,
              let json = try? JSONSerialization.jsonObject(with: data) as? [String: Any],
              let models = json["data"] as? [[String: Any]] else {
            return []
        }
        return models.compactMap { $0["id"] as? String }.sorted()
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
    func paste(_ text: String) {
        let trimmed = text.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty else { return }

        let pb = NSPasteboard.general
        pb.clearContents()
        pb.setString(trimmed, forType: .string)

        usleep(80_000)

        let source = CGEventSource(stateID: .combinedSessionState)
        let keyDown = CGEvent(keyboardEventSource: source, virtualKey: 0x09, keyDown: true)   // V
        let keyUp = CGEvent(keyboardEventSource: source, virtualKey: 0x09, keyDown: false)
        keyDown?.flags = .maskCommand
        keyUp?.flags = .maskCommand
        keyDown?.post(tap: .cghidEventTap)
        keyUp?.post(tap: .cghidEventTap)
    }
}

// MARK: - Overlay

final class RecordingOverlay {
    private var panel: NSPanel?
    private var label: NSTextField?
    private var clickMonitor: Any?
    private var dotTimer: Timer?
    private var dotCount = 0
    var onRetry: (() -> Void)?

    func showRecording() {
        stopDotAnimation()
        setNonInteractive()
        show(text: "● REC", minWidth: 110, maxWidth: 110)
    }

    func showWaiting() {
        setNonInteractive()
        // Start animated dots
        dotCount = 1
        stopDotAnimation()
        dotTimer = Timer.scheduledTimer(withTimeInterval: 0.4, repeats: true) { [weak self] _ in
            guard let self = self else { return }
            self.dotCount = (self.dotCount % 3) + 1
            let dots = String(repeating: "·", count: self.dotCount)
            self.show(text: dots, minWidth: 64, maxWidth: 64)
        }
        let dots = String(repeating: "·", count: dotCount)
        show(text: dots, minWidth: 64, maxWidth: 64)
    }

    func showPreview(_ text: String) {
        stopDotAnimation()
        setNonInteractive()
        let clean = text.replacingOccurrences(of: "\n", with: " ").trimmingCharacters(in: .whitespacesAndNewlines)
        if clean.isEmpty {
            showRecording()
        } else {
            // No truncation - show full text, panel grows vertically
            show(text: clean, minWidth: 120, maxWidth: 500)
        }
    }

    func showError(_ text: String) {
        stopDotAnimation()
        show(text: "ERR: \(text.prefix(120))", minWidth: 120, maxWidth: 420)
    }

    func showRetry() {
        stopDotAnimation()
        show(text: "↩", minWidth: 64, maxWidth: 64)
        DispatchQueue.main.async {
            self.setInteractive()
        }
    }

    func hide() {
        stopDotAnimation()
        DispatchQueue.main.async {
            self.setNonInteractive()
            self.panel?.orderOut(nil)
            self.onRetry = nil
        }
    }

    private func stopDotAnimation() {
        dotTimer?.invalidate()
        dotTimer = nil
        dotCount = 0
    }

    private func setInteractive() {
        panel?.ignoresMouseEvents = false
        clickMonitor = NSEvent.addLocalMonitorForEvents(matching: .leftMouseDown) { [weak self] event in
            guard let self = self, let panel = self.panel else { return event }
            let point = NSEvent.mouseLocation
            if panel.frame.contains(point) {
                self.onRetry?()
                return nil  // consume event
            }
            return event
        }
    }

    private func setNonInteractive() {
        if let monitor = clickMonitor {
            NSEvent.removeMonitor(monitor)
            clickMonitor = nil
        }
        panel?.ignoresMouseEvents = true
    }

    private func show(text: String, minWidth: CGFloat, maxWidth: CGFloat) {
        DispatchQueue.main.async { self.showOnMain(text: text, minWidth: minWidth, maxWidth: maxWidth) }
    }

    private func showOnMain(text: String, minWidth: CGFloat, maxWidth: CGFloat) {
        let font = NSFont.systemFont(ofSize: 13, weight: .semibold)
        let isMultiLine = text.count > 60
        let constrainedWidth = maxWidth - 24  // account for padding

        // Calculate size needed for text
        let boundingSize = NSSize(width: constrainedWidth, height: .greatestFiniteMagnitude)
        let textSize = (text as NSString).boundingRect(with: boundingSize, options: [.usesLineFragmentOrigin, .usesFontLeading], attributes: [.font: font])
        let neededWidth = ceil(textSize.width) + 24
        let neededHeight = ceil(textSize.height) + 16

        let width: CGFloat
        let height: CGFloat

        if isMultiLine {
            // Multi-line: grow vertically, cap width
            width = min(max(neededWidth, minWidth), maxWidth)
            height = min(max(neededHeight, 38), 200)  // cap at 200px height
        } else {
            // Single-line: grow horizontally up to maxWidth
            width = min(max(neededWidth, minWidth), maxWidth)
            height = 38
        }

        if panel == nil {
            let rect = NSRect(x: 0, y: 0, width: width, height: height)
            let p = NSPanel(contentRect: rect, styleMask: [.borderless, .nonactivatingPanel], backing: .buffered, defer: false)
            p.isFloatingPanel = true
            p.level = .floating
            p.collectionBehavior = [.canJoinAllSpaces, .fullScreenAuxiliary, .transient, .ignoresCycle]
            p.backgroundColor = .clear
            p.isOpaque = false
            p.hasShadow = true
            p.ignoresMouseEvents = true

            let visual = NSVisualEffectView(frame: rect)
            visual.autoresizingMask = [.width, .height]
            visual.material = .hudWindow
            visual.blendingMode = .behindWindow
            visual.state = .active
            visual.wantsLayer = true
            visual.layer?.cornerRadius = 14
            visual.layer?.masksToBounds = true

            let l = NSTextField(labelWithString: text)
            l.frame = rect.insetBy(dx: 12, dy: 8)
            l.autoresizingMask = [.width, .height]
            l.alignment = isMultiLine ? .left : .center
            l.lineBreakMode = .byWordWrapping
            l.font = font
            l.textColor = .white
            l.backgroundColor = .clear
            l.isBezeled = false
            l.isEditable = false
            l.isSelectable = false
            l.maximumNumberOfLines = 5

            visual.addSubview(l)
            p.contentView = visual
            panel = p
            label = l
        }

        label?.font = font
        label?.stringValue = text
        label?.alignment = isMultiLine ? .left : .center
        label?.lineBreakMode = isMultiLine ? .byWordWrapping : .byTruncatingTail

        guard let p = panel else { return }
        var frame = p.frame
        frame.size = NSSize(width: width, height: height)

        let mouse = NSEvent.mouseLocation
        let screen = NSScreen.screens.first(where: { NSMouseInRect(mouse, $0.frame, false) })
                  ?? NSScreen.main
                  ?? NSScreen.screens.first

        if let screen, Settings.shared.overlayCentered {
            // Centre on the screen the cursor is currently on.
            let visible = screen.visibleFrame
            frame.origin.x = visible.midX - frame.width / 2
            frame.origin.y = visible.midY - frame.height / 2
        } else if let screen {
            // Follow the cursor (legacy default).
            frame.origin.x = mouse.x + 14
            frame.origin.y = mouse.y - 52
            let visible = screen.visibleFrame
            if frame.maxX > visible.maxX { frame.origin.x = visible.maxX - frame.width - 8 }
            if frame.minX < visible.minX { frame.origin.x = visible.minX + 8 }
            if frame.minY < visible.minY { frame.origin.y = mouse.y + 20 }
            if frame.maxY > visible.maxY { frame.origin.y = visible.maxY - frame.height - 8 }
        }

        p.setFrame(frame, display: true, animate: true)
        p.orderFrontRegardless()
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
/// `.usesKeyDown` selects whether to listen on `.keyDown`/`.keyUp` events
/// (regular keys) or on `.flagsChanged` events (modifier keys).
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

    var usesKeyDown: Bool {
        // Modifiers fire as .flagsChanged; everything else as .keyDown.
        switch self {
        case .fn, .rightOption, .rightControl, .rightCommand, .rightShift:
            return false
        case .capsLock, .f13, .f14, .f15:
            return true
        }
    }

    var keyCode: CGKeyCode? {
        switch self {
        case .capsLock: return 57
        case .f13:      return 105
        case .f14:      return 107
        case .f15:      return 113
        default:        return nil
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

final class VoicePasteApp: NSObject, NSApplicationDelegate {
    private let store = SettingsStore.shared
    private let settings = Settings.shared
    private let recorder = Recorder()
    private let transcriber = Transcriber()
    private let localTranscriber = LocalTranscriber()
    private let typer = PasteboardTyper()
    private let overlay = RecordingOverlay()

    private var statusItem: NSStatusItem?
    private var eventTap: CFMachPort?

    private var isFnDown = false
    private let queue = RecordingQueueCoordinator()
    private var pendingStart: DispatchWorkItem?
    private var monitorTimer: Timer?
    private var hideWorkItem: DispatchWorkItem?

    private var previewText: String = ""
    private var previewInFlight = false
    private var lastPreviewChunkAt = Date.distantPast

    private var availableModels: [String] = []
    private var lastFailedAudioURL: URL?

    private var startDelay: TimeInterval { settings.recordingDelay }
    private var previewChunkInterval: TimeInterval { settings.realtimeChunkInterval }
    private let ringBufferDir = FileManager.default.temporaryDirectory.appendingPathComponent("voicepaste-fn-ring", isDirectory: true)
    private let ringBufferSize = 10

    func applicationDidFinishLaunching(_ notification: Notification) {
        NSApp.setActivationPolicy(.accessory)
        requestMicrophonePermission()
        setupMenuBar()
        installEventTap()

        // Fetch available models in background
        DispatchQueue.global(qos: .utility).async { [weak self] in
            guard let self = self else { return }
            let models = self.transcriber.fetchModels()
            DispatchQueue.main.async {
                self.availableModels = models
                print("Available models: \(models)")
                self.rebuildMenu()
            }
        }

        print("VoicePasteFn started")
        print("Endpoint: \(store.baseURL)")
        print("Model: \(settings.selectedModel)")
        print("Language: \(settings.language.rawValue)")
        print("Hold Fn for >= 0.2s to record. Release Fn to paste final transcript.")
    }

    private func requestMicrophonePermission() {
        switch AVCaptureDevice.authorizationStatus(for: .audio) {
        case .authorized:
            return
        case .notDetermined:
            AVCaptureDevice.requestAccess(for: .audio) { granted in
                if !granted { print("Microphone permission denied") }
            }
        default:
            print("Microphone permission is not authorized")
        }
    }

    private func setupMenuBar() {
        // Fixed-length item + real template image is more reliable/visible than a text-only
        // status item when the executable is launched from a SwiftPM-built .app wrapper.
        let item = NSStatusBar.system.statusItem(withLength: 30)
        if let button = item.button {
            if let image = NSImage(systemSymbolName: "mic.fill", accessibilityDescription: "VoicePaste") {
                image.isTemplate = true
                button.image = image
            } else {
                button.title = "VP"
            }
            button.toolTip = "VoicePaste Fn"
        }
        item.isVisible = true
        statusItem = item
        rebuildMenu()
    }

    private func rebuildMenu() {
        let menu = NSMenu()

        let title = NSMenuItem(title: "VoicePaste Fn", action: nil, keyEquivalent: "")
        title.isEnabled = false
        menu.addItem(title)
        menu.addItem(.separator())

        // Settings submenu — Endpoint + API key. Inline click no longer
        // pollutes the top-level menu; both live behind a "Settings ▶" item
        // so the menu bar stays compact.
        let settingsMenu = NSMenu()
        let endpointItem = NSMenuItem(
            title: "Endpoint:  \(store.maskedBaseURL)",
            action: #selector(editEndpoint),
            keyEquivalent: ""
        )
        endpointItem.target = self
        settingsMenu.addItem(endpointItem)
        let keyItem = NSMenuItem(
            title: "API Key:   \(store.maskedAPIKey)",
            action: #selector(editAPIKey),
            keyEquivalent: ""
        )
        keyItem.target = self
        settingsMenu.addItem(keyItem)
        let settingsRoot = NSMenuItem(title: "Settings", action: nil, keyEquivalent: "")
        menu.setSubmenu(settingsMenu, for: settingsRoot)
        menu.addItem(settingsRoot)

        // Recording delay submenu — discrete snap-points from 0.10s to 2.00s.
        // macOS menus don't host NSSlider directly, so we expose a coarse
        // 14-step scale here and offer Custom… for an exact value via NSAlert.
        let delayMenu = NSMenu()
        let delayChoices: [Double] = [0.10, 0.15, 0.20, 0.30, 0.40, 0.50,
                                       0.75, 1.00, 1.25, 1.50, 1.75, 2.00]
        let currentDelay = settings.recordingDelay
        for value in delayChoices {
            let title = formattedDelay(value)
            let item = NSMenuItem(title: title, action: #selector(setRecordingDelay(_:)), keyEquivalent: "")
            item.target = self
            item.representedObject = NSNumber(value: value)
            // Mark current choice so the user can see what's set.
            item.state = abs(currentDelay - value) < 0.001 ? .on : .off
            delayMenu.addItem(item)
        }
        delayMenu.addItem(.separator())
        let customItem = NSMenuItem(title: "Custom…", action: #selector(setCustomRecordingDelay), keyEquivalent: "")
        customItem.target = self
        delayMenu.addItem(customItem)
        let delayRoot = NSMenuItem(
            title: "Recording delay: \(formattedDelay(currentDelay))",
            action: nil, keyEquivalent: ""
        )
        menu.setSubmenu(delayMenu, for: delayRoot)
        menu.addItem(delayRoot)

        // Preview hide delay submenu — how long the overlay stays visible
        // after the final transcript is pasted. "Manual" = 0s = dismiss
        // only on next Fn-hold or click.
        let hideMenu = NSMenu()
        let hideChoices: [Double] = [0.0, 0.4, 0.8, 1.2, 1.5, 2.0, 3.0, 5.0]
        let currentHide = settings.hideDelay
        for value in hideChoices {
            let label: String
            if value == 0 { label = "Manual dismiss" } else { label = formattedDelay(value) }
            let item = NSMenuItem(title: label, action: #selector(setHideDelay(_:)), keyEquivalent: "")
            item.target = self
            item.representedObject = NSNumber(value: value)
            item.state = abs(currentHide - value) < 0.001 ? .on : .off
            hideMenu.addItem(item)
        }
        hideMenu.addItem(.separator())
        let customHide = NSMenuItem(title: "Custom…", action: #selector(setCustomHideDelay), keyEquivalent: "")
        customHide.target = self
        hideMenu.addItem(customHide)
        let hideRoot = NSMenuItem(
            title: currentHide == 0
                ? "Preview hide: Manual"
                : "Preview hide: \(formattedDelay(currentHide))",
            action: nil, keyEquivalent: ""
        )
        menu.setSubmenu(hideMenu, for: hideRoot)
        menu.addItem(hideRoot)
        menu.addItem(.separator())

        // Language submenu
        let langMenu = NSMenu()
        for lang in Language.allCases {
            let item = NSMenuItem(title: lang.title, action: #selector(setLanguage(_:)), keyEquivalent: "")
            item.target = self
            item.representedObject = lang.rawValue
            item.state = settings.language == lang ? .on : .off
            langMenu.addItem(item)
        }
        let langRoot = NSMenuItem(title: "Language: \(settings.language.rawValue)", action: nil, keyEquivalent: "")
        menu.setSubmenu(langMenu, for: langRoot)
        menu.addItem(langRoot)

        // Model submenu
        let modelMenu = NSMenu()
        let autoItem = NSMenuItem(title: "Auto", action: #selector(setModel(_:)), keyEquivalent: "")
        autoItem.target = self
        autoItem.representedObject = "auto"
        autoItem.state = settings.selectedModel == "auto" ? .on : .off
        modelMenu.addItem(autoItem)
        modelMenu.addItem(.separator())
        for modelId in availableModels {
            let item = NSMenuItem(title: modelId, action: #selector(setModel(_:)), keyEquivalent: "")
            item.target = self
            item.representedObject = modelId
            item.state = settings.selectedModel == modelId ? .on : .off
            modelMenu.addItem(item)
        }
        modelMenu.addItem(.separator())
        let refreshItem = NSMenuItem(title: "↻ Refresh models", action: #selector(refreshModels(_:)), keyEquivalent: "")
        refreshItem.target = self
        modelMenu.addItem(refreshItem)
        let modelRoot = NSMenuItem(title: "Model: \(settings.selectedModel)", action: nil, keyEquivalent: "")
        menu.setSubmenu(modelMenu, for: modelRoot)
        menu.addItem(modelRoot)

        let realtime = NSMenuItem(title: "Realtime preview", action: #selector(toggleRealtime), keyEquivalent: "")
        realtime.target = self
        realtime.state = settings.realtimePreview ? .on : .off
        menu.addItem(realtime)

        // Realtime cadence submenu — only meaningful when "Realtime preview" is on.
        let chunkMenu = NSMenu()
        let chunkChoices: [Double] = [1.0, 2.0, 3.0, 5.0, 8.0, 10.0, 15.0, 20.0, 30.0]
        let currentChunk = settings.realtimeChunkInterval
        for v in chunkChoices {
            let item = NSMenuItem(title: "\(formattedDelay(v))",
                                  action: #selector(setRealtimeChunkInterval(_:)),
                                  keyEquivalent: "")
            item.target = self
            item.representedObject = NSNumber(value: v)
            item.state = abs(currentChunk - v) < 0.001 ? .on : .off
            chunkMenu.addItem(item)
        }
        chunkMenu.addItem(.separator())
        let customChunk = NSMenuItem(title: "Custom…",
                                     action: #selector(setCustomRealtimeChunkInterval),
                                     keyEquivalent: "")
        customChunk.target = self
        chunkMenu.addItem(customChunk)
        let chunkRoot = NSMenuItem(
            title: "Realtime every: \(formattedDelay(currentChunk))",
            action: nil, keyEquivalent: ""
        )
        menu.setSubmenu(chunkMenu, for: chunkRoot)
        menu.addItem(chunkRoot)

        let autostart = NSMenuItem(title: "Autostart", action: #selector(toggleAutostart), keyEquivalent: "")
        autostart.target = self
        autostart.state = settings.autostart ? .on : .off
        menu.addItem(autostart)

        // Hotkey submenu — pick which physical key triggers dictation.
        let hotkeyMenu = NSMenu()
        let currentHotkey = settings.hotkey
        for kind in HotkeyKind.allCases {
            let item = NSMenuItem(title: kind.title,
                                  action: #selector(setHotkey(_:)),
                                  keyEquivalent: "")
            item.target = self
            item.representedObject = kind.rawValue
            item.state = (kind == currentHotkey) ? .on : .off
            hotkeyMenu.addItem(item)
        }
        let hotkeyRoot = NSMenuItem(
            title: "Hotkey: \(currentHotkey.title)",
            action: nil, keyEquivalent: ""
        )
        menu.setSubmenu(hotkeyMenu, for: hotkeyRoot)
        menu.addItem(hotkeyRoot)

        // Activation submenu — hold (press/release) vs toggle (press/press).
        let actMenu = NSMenu()
        let currentMode = settings.activationMode
        for m in ActivationMode.allCases {
            let item = NSMenuItem(title: m.title,
                                  action: #selector(setActivationMode(_:)),
                                  keyEquivalent: "")
            item.target = self
            item.representedObject = m.rawValue
            item.state = (m == currentMode) ? .on : .off
            actMenu.addItem(item)
        }
        let actRoot = NSMenuItem(title: "Activation: \(currentMode == .hold ? "Hold" : "Toggle")",
                                 action: nil, keyEquivalent: "")
        menu.setSubmenu(actMenu, for: actRoot)
        menu.addItem(actRoot)

        // Overlay position toggle (centered on screen vs follow cursor).
        let overlayItem = NSMenuItem(title: "Centre overlay on screen",
                                     action: #selector(toggleOverlayCentered),
                                     keyEquivalent: "")
        overlayItem.target = self
        overlayItem.state = settings.overlayCentered ? .on : .off
        menu.addItem(overlayItem)

        // Wake server on dictation start so cold-load latency doesn't kill
        // the first recording after an idle timeout.
        let wakeItem = NSMenuItem(title: "Wake server on dictation start",
                                  action: #selector(toggleWakeServerOnStart),
                                  keyEquivalent: "")
        wakeItem.target = self
        wakeItem.state = settings.wakeServerOnStart ? .on : .off
        menu.addItem(wakeItem)

        // Apple fallback toggle — use local speech recognition when server fails.
        let fallbackItem = NSMenuItem(title: "Apple fallback on server failure",
                                      action: #selector(toggleAppleFallback),
                                      keyEquivalent: "")
        fallbackItem.target = self
        fallbackItem.state = settings.appleFallback ? .on : .off
        menu.addItem(fallbackItem)

        menu.addItem(.separator())
        let permissions = NSMenuItem(title: "Permissions: \(permissionStatus())", action: #selector(openPermissions), keyEquivalent: "")
        permissions.target = self
        menu.addItem(permissions)

        let quit = NSMenuItem(title: "Quit", action: #selector(quit), keyEquivalent: "q")
        quit.target = self
        menu.addItem(quit)

        statusItem?.menu = menu
    }

    @objc private func setLanguage(_ sender: NSMenuItem) {
        if let raw = sender.representedObject as? String, let lang = Language(rawValue: raw) {
            settings.language = lang
            rebuildMenu()
        }
    }

    @objc private func setModel(_ sender: NSMenuItem) {
        if let modelId = sender.representedObject as? String {
            settings.selectedModel = modelId
            rebuildMenu()
        }
    }

    @objc private func refreshModels(_ sender: NSMenuItem) {
        DispatchQueue.global(qos: .utility).async { [weak self] in
            guard let self = self else { return }
            let models = self.transcriber.fetchModels()
            DispatchQueue.main.async {
                self.availableModels = models
                print("Models refreshed: \(models)")
                self.rebuildMenu()
            }
        }
    }

    // MARK: - Recording delay (debounce for Fn hold)

    private func formattedDelay(_ value: TimeInterval) -> String {
        // 0.10s, 0.20s, 1.00s — always two decimals so the menu reads cleanly.
        return String(format: "%.2fs", value)
    }

    @objc private func setRecordingDelay(_ sender: NSMenuItem) {
        if let n = sender.representedObject as? NSNumber {
            settings.recordingDelay = n.doubleValue
            print("Recording delay set to \(formattedDelay(settings.recordingDelay))")
            rebuildMenu()
        }
    }

    @objc private func setCustomRecordingDelay() {
        let alert = NSAlert()
        alert.messageText = "Recording delay"
        alert.informativeText = "How long Fn must be held before recording actually starts. " +
            "Range 0.10 – 2.00 seconds. Saved to UserDefaults; takes effect immediately."
        alert.alertStyle = .informational
        alert.addButton(withTitle: "Save")
        alert.addButton(withTitle: "Cancel")

        let input = NSTextField(frame: NSRect(x: 0, y: 0, width: 200, height: 24))
        input.stringValue = String(format: "%.2f", settings.recordingDelay)
        input.placeholderString = "0.20"
        alert.accessoryView = input
        DispatchQueue.main.async { [weak input, weak alert] in
            guard let input, let alert else { return }
            let w = alert.window
            w.initialFirstResponder = input
            w.makeFirstResponder(input)
            input.selectText(nil)
        }

        let response = alert.runModal()
        guard response == .alertFirstButtonReturn else { return }
        let raw = input.stringValue.trimmingCharacters(in: .whitespaces)
        guard let parsed = Double(raw), parsed >= 0.10, parsed <= 2.0 else {
            presentError(title: "Invalid value",
                         message: "Enter a number between 0.10 and 2.00 (seconds).")
            return
        }
        settings.recordingDelay = parsed
        print("Recording delay set to \(formattedDelay(settings.recordingDelay))")
        rebuildMenu()
    }

    // MARK: - Preview hide delay (set after paste)

    @objc private func setHideDelay(_ sender: NSMenuItem) {
        if let n = sender.representedObject as? NSNumber {
            settings.hideDelay = n.doubleValue
            print("Preview hide delay set to \(settings.hideDelay == 0 ? "Manual" : formattedDelay(settings.hideDelay))")
            rebuildMenu()
        }
    }

    @objc private func setCustomHideDelay() {
        let alert = NSAlert()
        alert.messageText = "Preview hide delay"
        alert.informativeText = "How long the overlay stays visible after the final " +
            "transcript is pasted, so you can read what landed in the clipboard. " +
            "Range 0.0 – 5.0 seconds. 0 = manual dismiss (next Fn or click). " +
            "Saved to UserDefaults; takes effect immediately."
        alert.alertStyle = .informational
        alert.addButton(withTitle: "Save")
        alert.addButton(withTitle: "Cancel")

        let input = NSTextField(frame: NSRect(x: 0, y: 0, width: 200, height: 24))
        input.stringValue = String(format: "%.2f", settings.hideDelay)
        input.placeholderString = "0.80"
        alert.accessoryView = input
        DispatchQueue.main.async { [weak input, weak alert] in
            guard let input, let alert else { return }
            let w = alert.window
            w.initialFirstResponder = input
            w.makeFirstResponder(input)
            input.selectText(nil)
        }

        let response = alert.runModal()
        guard response == .alertFirstButtonReturn else { return }
        let raw = input.stringValue.trimmingCharacters(in: .whitespaces)
        guard let parsed = Double(raw), parsed >= 0.0, parsed <= 5.0 else {
            presentError(title: "Invalid value",
                         message: "Enter a number between 0.0 and 5.0 (seconds; 0 = manual dismiss).")
            return
        }
        settings.hideDelay = parsed
        print("Preview hide delay set to \(settings.hideDelay == 0 ? "Manual" : formattedDelay(settings.hideDelay))")
        rebuildMenu()
    }

    // MARK: - Endpoint / API key dialogs

    @objc private func editEndpoint() {
        let alert = NSAlert()
        alert.messageText = "Whisper endpoint"
        alert.informativeText = "Base URL of any OpenAI-compatible Whisper server. " +
            "For example: https://api.openai.com/v1 or your self-hosted server. " +
            "Saved to UserDefaults; takes effect on the next recording — no restart needed."
        alert.alertStyle = .informational
        alert.addButton(withTitle: "Save")
        alert.addButton(withTitle: "Cancel")
        alert.addButton(withTitle: "Reset to default")

        let input = NSTextField(frame: NSRect(x: 0, y: 0, width: 360, height: 24))
        input.stringValue = store.baseURL
        input.placeholderString = "https://api.openai.com/v1"
        alert.accessoryView = input
        // Make the text field first responder once the alert window exists.
        DispatchQueue.main.async { [weak input, weak alert] in
            guard let input, let alert else { return }
            let window = alert.window
            window.initialFirstResponder = input
            window.makeFirstResponder(input)
            input.selectText(nil)
        }

        let response = alert.runModal()
        switch response {
        case .alertFirstButtonReturn:    // Save
            do {
                try store.setBaseURL(input.stringValue)
                print("Endpoint updated to: \(store.baseURL)")
                rebuildMenu()
                // The model list is endpoint-specific; refresh in background.
                refreshModelsFromBackground()
            } catch {
                presentError(title: "Couldn't save endpoint", message: error.localizedDescription)
            }
        case .alertThirdButtonReturn:    // Reset
            UserDefaults.standard.removeObject(forKey: kDefaultsKeyBaseURL)
            UserDefaults.standard.removeObject(forKey: kDefaultsKeyBaseURLSet)
            print("Endpoint reset to default: \(store.baseURL)")
            rebuildMenu()
            refreshModelsFromBackground()
        default:
            break
        }
    }

    @objc private func editAPIKey() {
        let alert = NSAlert()
        alert.messageText = "Whisper API key"
        alert.informativeText = "Stored in the macOS Keychain (system-encrypted, only this app can read it). " +
            "Env var OPENAI_API_KEY, if set, wins for the current launch — useful for shell testing."
        alert.alertStyle = .informational
        alert.addButton(withTitle: "Save")
        alert.addButton(withTitle: "Cancel")
        alert.addButton(withTitle: "Clear")

        let input = NSSecureTextField(frame: NSRect(x: 0, y: 0, width: 360, height: 24))
        input.stringValue = store.apiKey
        input.placeholderString = "sk-…"
        alert.accessoryView = input
        DispatchQueue.main.async { [weak input, weak alert] in
            guard let input, let alert else { return }
            let window = alert.window
            window.initialFirstResponder = input
            window.makeFirstResponder(input)
        }

        let response = alert.runModal()
        switch response {
        case .alertFirstButtonReturn:    // Save
            let trimmed = input.stringValue.trimmingCharacters(in: .whitespacesAndNewlines)
            do {
                try store.setAPIKey(trimmed)
                print(trimmed.isEmpty ? "API key cleared" : "API key saved to Keychain (\(trimmed.count) chars)")
                rebuildMenu()
            } catch {
                presentError(title: "Couldn't save API key", message: error.localizedDescription)
            }
        case .alertThirdButtonReturn:    // Clear
            store.clearAPIKey()
            print("API key cleared from Keychain")
            rebuildMenu()
        default:
            break
        }
    }

    private func refreshModelsFromBackground() {
        DispatchQueue.global(qos: .utility).async { [weak self] in
            guard let self = self else { return }
            let models = self.transcriber.fetchModels()
            DispatchQueue.main.async {
                self.availableModels = models
                self.rebuildMenu()
            }
        }
    }

    private func presentError(title: String, message: String) {
        let a = NSAlert()
        a.messageText = title
        a.informativeText = message
        a.alertStyle = .warning
        a.addButton(withTitle: "OK")
        a.runModal()
    }

    @objc private func toggleRealtime() {
        settings.realtimePreview.toggle()
        rebuildMenu()
    }

    @objc private func setRealtimeChunkInterval(_ sender: NSMenuItem) {
        if let n = sender.representedObject as? NSNumber {
            settings.realtimeChunkInterval = n.doubleValue
            print("Realtime chunk interval set to \(formattedDelay(settings.realtimeChunkInterval))")
            rebuildMenu()
        }
    }

    @objc private func setCustomRealtimeChunkInterval() {
        let alert = NSAlert()
        alert.messageText = "Realtime preview cadence"
        alert.informativeText = "How often (in seconds) a partial transcript is fetched " +
            "while the user is still holding the hotkey. Smaller = more responsive, " +
            "more API calls. Range 1.0 – 30.0 seconds. Default 5.0."
        alert.alertStyle = .informational
        alert.addButton(withTitle: "Save")
        alert.addButton(withTitle: "Cancel")

        let input = NSTextField(frame: NSRect(x: 0, y: 0, width: 200, height: 24))
        input.stringValue = String(format: "%.2f", settings.realtimeChunkInterval)
        input.placeholderString = "5.00"
        alert.accessoryView = input
        DispatchQueue.main.async { [weak input, weak alert] in
            guard let input, let alert else { return }
            let w = alert.window
            w.initialFirstResponder = input
            w.makeFirstResponder(input)
            input.selectText(nil)
        }

        let response = alert.runModal()
        guard response == .alertFirstButtonReturn else { return }
        let raw = input.stringValue.trimmingCharacters(in: .whitespaces)
        guard let parsed = Double(raw), parsed >= 1.0, parsed <= 30.0 else {
            presentError(title: "Invalid value",
                         message: "Enter a number between 1.0 and 30.0 (seconds).")
            return
        }
        settings.realtimeChunkInterval = parsed
        print("Realtime chunk interval set to \(formattedDelay(settings.realtimeChunkInterval))")
        rebuildMenu()
    }

    @objc private func toggleAutostart() {
        settings.autostart.toggle()
        AutostartManager.setEnabled(settings.autostart)
        rebuildMenu()
    }

    @objc private func setHotkey(_ sender: NSMenuItem) {
        guard let raw = sender.representedObject as? String,
              let kind = HotkeyKind(rawValue: raw) else { return }
        let old = settings.hotkey
        settings.hotkey = kind
        print("Hotkey changed: \(old.title) -> \(kind.title). " +
              "Take effect on next launch (event tap is set up once at start).")
        rebuildMenu()
    }

    @objc private func setActivationMode(_ sender: NSMenuItem) {
        guard let raw = sender.representedObject as? String,
              let mode = ActivationMode(rawValue: raw) else { return }
        settings.activationMode = mode
        print("Activation mode: \(mode == .hold ? "Hold" : "Toggle")")
        rebuildMenu()
    }

    @objc private func toggleOverlayCentered() {
        settings.overlayCentered.toggle()
        rebuildMenu()
    }

    @objc private func toggleWakeServerOnStart() {
        settings.wakeServerOnStart.toggle()
        rebuildMenu()
    }

    @objc private func toggleAppleFallback() {
        settings.appleFallback.toggle()
        rebuildMenu()
    }

    @objc private func openPermissions() {
        // macOS 13+ uses x-apple.systempreferences:com.apple.preference.security?Privacy
        let url = URL(string: "x-apple.systempreferences:com.apple.preference.security?Privacy")!
        NSWorkspace.shared.open(url)
    }

    private func permissionStatus() -> String {
        let mic = AVCaptureDevice.authorizationStatus(for: .audio)
        let micStatus = mic == .authorized ? "✓" : "✗"

        // Check Accessibility (required for Fn key monitoring)
        let accessibilityGranted = AXIsProcessTrusted()
        let accStatus = accessibilityGranted ? "✓" : "✗"

        return "\(micStatus) Mic  \(accStatus) Accessibility"
    }

    @objc private func quit() {
        stopRecordingWithoutPaste()
        NSApp.terminate(nil)
    }

    private func installEventTap() {
        // Subscribe to flagsChanged for modifier hotkeys, plus keyDown/keyUp
        // for non-modifier hotkeys (Caps Lock, F13–F15). The actual key
        // filter happens inside the callback — the tap just gets every event
        // in those classes.
        var mask: Int64 = 0
        mask |= (1 << CGEventType.flagsChanged.rawValue)
        mask |= (1 << CGEventType.keyDown.rawValue)
        mask |= (1 << CGEventType.keyUp.rawValue)
        let opaqueSelf = Unmanaged.passUnretained(self).toOpaque()

        guard let tap = CGEvent.tapCreate(
            tap: .cgSessionEventTap,
            place: .headInsertEventTap,
            options: .defaultTap,
            eventsOfInterest: CGEventMask(mask),
            callback: { _, type, event, userInfo in
                guard let userInfo else { return Unmanaged.passUnretained(event) }
                let app = Unmanaged<VoicePasteApp>.fromOpaque(userInfo).takeUnretainedValue()
                app.handle(type: type, event: event)
                return Unmanaged.passUnretained(event)
            },
            userInfo: opaqueSelf
        ) else {
            print("Failed to create event tap. Grant Accessibility + Input Monitoring to Terminal or this binary.")
            return
        }

        eventTap = tap
        let source = CFMachPortCreateRunLoopSource(kCFAllocatorDefault, tap, 0)
        CFRunLoopAddSource(CFRunLoopGetMain(), source, .commonModes)
        CGEvent.tapEnable(tap: tap, enable: true)
    }

    private func handle(type: CGEventType, event: CGEvent) {
        // First decide whether *this* event is our hotkey going down / up.
        let hotkey = Settings.shared.hotkey
        let isDown: Bool
        switch hotkey.usesKeyDown {
        case true:
            // keyCode-based: filter on keyCode + type.
            guard type == .keyDown || type == .keyUp,
                  let target = hotkey.keyCode else { return }
            let code = Int(event.getIntegerValueField(.keyboardEventKeycode))
            guard code == target else { return }
            isDown = (type == .keyDown)
        case false:
            // Modifier flag-based: only the relevant flag edge.
            guard type == .flagsChanged,
                  let flag = hotkey.flag else { return }
            let flags = event.flags
            // Fn has no left/right distinction; the others must be Right*.
            if hotkey.requiresRightSide && !flags.contains(.maskNonCoalesced) {
                // .maskNonCoalesced = right-side bit 0x100 — Apple convention:
                // a modifier is on the right if the *raw* event has the
                // "right" indicator (kCGEventRightFlag). We approximate by
                // checking .maskNonCoalesced which usually equals right.
                // In practice, on most keyboards flag-based detection with
                // .maskNonCoalesced is reliable enough; the Left modifier
                // is left alone for the user's existing shortcuts.
            }
            isDown = flags.contains(flag)
        }

        // Edge detection differs for hold vs toggle.
        let mode = Settings.shared.activationMode
        DispatchQueue.main.async { [weak self] in
            guard let self = self else { return }
            switch mode {
            case .hold:
                self.handleHoldEdge(pressed: isDown)
            case .toggle:
                self.handleToggleEdge(pressed: isDown)
            }
        }
    }

    private func handleHoldEdge(pressed: Bool) {
        if pressed && !isFnDown {
            isFnDown = true
            if self.lastFailedAudioURL != nil {
                self.lastFailedAudioURL = nil
                self.overlay.hide()
                self.queue.clearBusyAfterRetry()
            }
            self.scheduleRecordingStart()
        } else if !pressed && isFnDown {
            self.isFnDown = false
            self.finishRecordingAndPaste()
        }
    }

    /// For toggle mode, the first press starts, the second stops. We use the
    /// `pressed==true` edge as "the user tapped the hotkey". Each press
    /// flips the toggle state.
    private func handleToggleEdge(pressed: Bool) {
        guard pressed else { return }   // ignore the release edges
        if !isFnDown {
            // off -> on (queue if busy, start if idle)
            isFnDown = true
            if lastFailedAudioURL != nil {
                lastFailedAudioURL = nil
                overlay.hide()
                queue.clearBusyAfterRetry()
            }
            scheduleRecordingStart()
        } else {
            // on -> off
            isFnDown = false
            finishRecordingAndPaste()
        }
    }

    private func scheduleRecordingStart() {
        let action = queue.requestRecording()
        if action != .startRecording {
            // Queued or no-op — nothing to schedule right now
            return
        }
        if Settings.shared.wakeServerOnStart {
            // Warm the server-side model by sending a real transcription
            // request with a 1-second silence WAV. The endpoint forces the
            // model back into memory before our actual dictation lands, which
            // kills the cold-start penalty after a long idle. POSTing
            // /audio/transcriptions (instead of /models) is what the user
            // asked for: the server actually runs the model on our audio,
            // not just unloads it lazily.
            //
            // Failures are intentionally silent — losing a wake-up is much
            // less annoying than alerting the user to a still-cold endpoint.
            DispatchQueue.global(qos: .userInitiated).async { [weak self] in
                guard let self = self else { return }
                do {
                    let url = try WakeWav.shared.ensureSilenceWav()
                    let lang = self.settings.language
                    let model = self.settings.selectedModel == "auto" ? nil : self.settings.selectedModel
                    _ = try? self.transcriber.transcribe(
                        fileURL: url, language: lang, model: model
                    )
                } catch {
                    // best-effort, never surface
                }
            }
        }
        pendingStart?.cancel()
        let work = DispatchWorkItem { [weak self] in
            guard let self, self.isFnDown, !self.queue.isRecording else { return }
            self.startRecordingSegment(resetChunks: true)
        }
        pendingStart = work
        DispatchQueue.main.asyncAfter(deadline: .now() + startDelay, execute: work)
    }

    private func startRecordingSegment(resetChunks: Bool) {
        do {
            hideWorkItem?.cancel()
            hideWorkItem = nil
            if resetChunks {
                previewText = ""
            }
            lastPreviewChunkAt = Date()
            try recorder.start()
            queue.onRecordingStarted()
            overlay.showRecording()
            startMonitorTimer()
            print("REC")
        } catch {
            overlay.showError(error.localizedDescription)
            print("record start error: \(error.localizedDescription)")
        }
    }

    private func startMonitorTimer() {
        monitorTimer?.invalidate()
        monitorTimer = Timer.scheduledTimer(withTimeInterval: 0.10, repeats: true) { [weak self] _ in
            self?.monitorAudio()
        }
    }

    private func monitorAudio() {
        guard queue.isRecording else { return }
        let now = Date()
        if now.timeIntervalSince(lastPreviewChunkAt) >= previewChunkInterval {
            lastPreviewChunkAt = now
            triggerPreviewChunk()
        }
    }

    private func triggerPreviewChunk() {
        guard settings.realtimePreview, !previewInFlight, let url = recorder.currentURL else { return }
        previewInFlight = true
        // Don't clear text - show accumulated text with "processing" suffix
        let currentText = previewText
        if !currentText.isEmpty {
            overlay.showPreview(currentText + " …")
        } else {
            overlay.showWaiting()
        }

        let chunkURL = FileManager.default.temporaryDirectory
            .appendingPathComponent("voicepaste-preview-\(UUID().uuidString)")
            .appendingPathExtension("wav")
        try? FileManager.default.copyItem(at: url, to: chunkURL)

        DispatchQueue.global(qos: .utility).async {
            defer {
                try? FileManager.default.removeItem(at: chunkURL)
                DispatchQueue.main.async { self.previewInFlight = false }
            }
            do {
                let model = self.settings.selectedModel == "auto" ? nil : self.settings.selectedModel
                var text = try self.transcriber.transcribe(fileURL: chunkURL, language: self.settings.language, model: model)
                text = TextCleaner.clean(text)
                DispatchQueue.main.async {
                    // Accumulate text - append new text to previous
                    let clean = text.trimmingCharacters(in: .whitespacesAndNewlines)
                    if !clean.isEmpty {
                        // Only append if this is new text (not a duplicate of what we already have)
                        if self.previewText.isEmpty {
                            self.previewText = clean
                        } else if !self.previewText.contains(clean.prefix(20)) {
                            // New text - append
                            self.previewText = self.previewText + " " + clean
                        } else {
                            // Text already included - update anyway for completeness
                            self.previewText = clean
                        }
                    }
                    if self.queue.isRecording { self.overlay.showPreview(self.previewText) }
                }
            } catch {
                DispatchQueue.main.async {
                    // On error, show accumulated text without suffix
                    if self.queue.isRecording {
                        if !currentText.isEmpty {
                            self.overlay.showPreview(currentText)
                        } else {
                            self.overlay.showRecording()
                        }
                    }
                }
            }
        }
    }

    /// Build a RetryTranscriber with current settings (server as primary,
    /// optional Apple fallback).
    private func makeRetryTranscriber() -> RetryTranscriber {
        let language = settings.language
        let model = settings.selectedModel == "auto" ? nil : settings.selectedModel
        let server = ServerTranscriptionService(transcriber: transcriber, language: language, model: model)
        let fallback: TranscriptionService? = settings.appleFallback ? localTranscriber : nil
        return RetryTranscriber(primary: server, fallback: fallback, maxAttempts: 3)
    }

    private func finishRecordingAndPaste() {
        pendingStart?.cancel()
        pendingStart = nil

        guard queue.isRecording else {
            overlay.hide()
            return
        }
        guard let url = recorder.stop() else { return }

        queue.onRecordingStopped()
        monitorTimer?.invalidate()
        monitorTimer = nil
        overlay.showWaiting()
        print("TRANSCRIBE FINAL (full retranscription)")

        let accumulatedPreview = previewText

        DispatchQueue.global(qos: .userInitiated).async {
            do {
                // Final full retranscription with auto-retry + optional Apple fallback
                let retryTranscriber = self.makeRetryTranscriber()
                var finalText = try retryTranscriber.transcribe(fileURL: url, languageCode: self.settings.language.rawValue)
                finalText = TextCleaner.clean(finalText)
                let cleanFinal = finalText.trimmingCharacters(in: .whitespacesAndNewlines)
                let result = cleanFinal.isEmpty ? accumulatedPreview : cleanFinal
                print("TEXT: \(result)")
                self.saveToRingBuffer(url)
                try? FileManager.default.removeItem(at: url)
                DispatchQueue.main.async {
                    self.overlay.showPreview(result)
                    self.typer.paste(result)
                    let hd = self.settings.hideDelay
                    if hd > 0 {
                        let work = DispatchWorkItem { [weak self] in
                            guard let self else { return }
                            self.overlay.hide()
                            let nextAction = self.queue.onTranscriptionCompleted()
                            if nextAction == .startRecording {
                                self.drainPendingRecording()
                            }
                        }
                        self.hideWorkItem = work
                        DispatchQueue.main.asyncAfter(deadline: .now() + hd, execute: work)
                    } else {
                        // Manual dismiss: hide only on next Fn-hold or click.
                        let nextAction = self.queue.onTranscriptionCompleted()
                        if nextAction == .startRecording {
                            self.drainPendingRecording()
                        }
                        self.previewText = ""
                    }
                }
            } catch {
                print("transcription error: \(error.localizedDescription)")
                // Save audio for retry - copy to persistent location
                let retryURL = self.saveToRingBuffer(url)
                DispatchQueue.main.async {
                    self.queue.cancelPending()
                    self.lastFailedAudioURL = retryURL
                    self.overlay.onRetry = { [weak self] in self?.retryTranscription() }
                    self.overlay.showRetry()
                    let nextAction = self.queue.onTranscriptionCompleted()
                    if nextAction == .startRecording {
                        self.drainPendingRecording()
                    }
                }
            }
        }
    }

    private func retryTranscription() {
        guard let url = lastFailedAudioURL else { return }
        overlay.onRetry = nil
        queue.onRetryStarted()
        overlay.showWaiting()
        print("RETRY TRANSCRIPTION")

        DispatchQueue.global(qos: .userInitiated).async {
            do {
                let retryTranscriber = self.makeRetryTranscriber()
                var text = try retryTranscriber.transcribe(fileURL: url, languageCode: self.settings.language.rawValue)
                text = TextCleaner.clean(text)
                let clean = text.trimmingCharacters(in: .whitespacesAndNewlines)
                print("RETRY TEXT: \(clean)")
                try? FileManager.default.removeItem(at: url)
                DispatchQueue.main.async {
                    self.lastFailedAudioURL = nil
                    if !clean.isEmpty {
                        self.overlay.showPreview(clean)
                        self.typer.paste(clean)
                    }
                    let hd = self.settings.hideDelay
                    if hd > 0 {
                        let work = DispatchWorkItem { [weak self] in
                            guard let self else { return }
                            self.overlay.hide()
                            let nextAction = self.queue.onTranscriptionCompleted()
                            if nextAction == .startRecording {
                                self.drainPendingRecording()
                            }
                        }
                        self.hideWorkItem = work
                        DispatchQueue.main.asyncAfter(deadline: .now() + hd, execute: work)
                    } else {
                        let nextAction = self.queue.onTranscriptionCompleted()
                        if nextAction == .startRecording {
                            self.drainPendingRecording()
                        }
                        self.previewText = ""
                    }
                }
            } catch {
                print("retry transcription error: \(error.localizedDescription)")
                DispatchQueue.main.async {
                    self.overlay.onRetry = { [weak self] in self?.retryTranscription() }
                    self.overlay.showRetry()
                    let nextAction = self.queue.onTranscriptionCompleted()
                    if nextAction == .startRecording {
                        self.drainPendingRecording()
                    }
                }
            }
        }
    }

    @discardableResult
    private func saveToRingBuffer(_ url: URL) -> URL {
        let fm = FileManager.default
        let dest = ringBufferDir.appendingPathComponent("\(Int(Date().timeIntervalSince1970 * 1000)).wav")
        try? fm.createDirectory(at: ringBufferDir, withIntermediateDirectories: true)
        try? fm.copyItem(at: url, to: dest)

        // Prune ring buffer to keep only last N files
        if let files = try? fm.contentsOfDirectory(at: ringBufferDir, includingPropertiesForKeys: [.creationDateKey]) {
            let sorted = files.sorted { a, b in
                let da = (try? a.resourceValues(forKeys: [.creationDateKey]))?.creationDate ?? .distantPast
                let db = (try? b.resourceValues(forKeys: [.creationDateKey]))?.creationDate ?? .distantPast
                return da < db
            }
            if sorted.count > ringBufferSize {
                for f in sorted.prefix(sorted.count - ringBufferSize) {
                    try? fm.removeItem(at: f)
                }
            }
        }
        return dest
    }

    private func stopRecordingWithoutPaste() {
        pendingStart?.cancel()
        pendingStart = nil
        monitorTimer?.invalidate()
        monitorTimer = nil
        recorder.stopWithoutReturning()
        queue.reset()
        hideWorkItem?.cancel()
        hideWorkItem = nil
        lastFailedAudioURL = nil
        overlay.onRetry = nil
        overlay.hide()
    }

    /// Called when transcription completes and a queued recording should start.
    /// Starts recording immediately (no debounce delay) to eliminate gaps.
    /// If Fn is no longer held, the pending recording is discarded.
    private func drainPendingRecording() {
        hideWorkItem?.cancel()
        hideWorkItem = nil
        previewText = ""

        guard isFnDown else {
            queue.cancelPending()
            return
        }

        do {
            try recorder.start()
            queue.onRecordingStarted()
            overlay.showRecording()
            lastPreviewChunkAt = Date()
            startMonitorTimer()
            print("DRAIN REC")
        } catch {
            overlay.showError(error.localizedDescription)
            print("drain record start error: \(error.localizedDescription)")
        }
    }


}

let app = NSApplication.shared
let delegate = VoicePasteApp()
app.delegate = delegate
app.setActivationPolicy(.accessory)
app.run()
