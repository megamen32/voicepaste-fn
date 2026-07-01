import AppKit
import AVFoundation
import ApplicationServices
import Foundation
import Security

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

extension Data {
    mutating func appendString(_ string: String) {
        append(string.data(using: .utf8)!)
    }
}

// MARK: - Text Cleanup

final class TextCleaner {
    private static let unwantedSuffixes = [
        "продолжение следует",
        "субтитры сделал DimaTorzok",
        "субтитры сделаны DimaTorzok",
        "subtitles by DimaTorzok",
        "subtitles made by DimaTorzok",
        "продолжение следует...",
        "to be continued",
        "to be continued...",
    ]

    static func clean(_ text: String) -> String {
        var result = text
        for suffix in unwantedSuffixes {
            let lowercased = result.lowercased()
            let lowerSuffix = suffix.lowercased()
            if lowercased.hasSuffix(lowerSuffix) {
                let cutIndex = result.index(result.endIndex, offsetBy: -suffix.count)
                result = String(result[..<cutIndex])
                    .trimmingCharacters(in: .whitespacesAndNewlines)
                    .trimmingCharacters(in: CharacterSet(charactersIn: "."))
                    .trimmingCharacters(in: .whitespacesAndNewlines)
            }
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
        frame.origin.x = mouse.x + 14
        frame.origin.y = mouse.y - 52

        if let screen = NSScreen.screens.first(where: { NSMouseInRect(mouse, $0.frame, false) }) ?? NSScreen.main {
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

// MARK: - App

final class VoicePasteApp: NSObject, NSApplicationDelegate {
    private let store = SettingsStore.shared
    private let settings = Settings.shared
    private let recorder = Recorder()
    private let transcriber = Transcriber()
    private let typer = PasteboardTyper()
    private let overlay = RecordingOverlay()

    private var statusItem: NSStatusItem?
    private var eventTap: CFMachPort?

    private var isFnDown = false
    private var isRecording = false
    private var isBusy = false
    private var pendingStart: DispatchWorkItem?
    private var monitorTimer: Timer?

    private var previewText: String = ""
    private var previewInFlight = false
    private var lastPreviewChunkAt = Date.distantPast

    private var availableModels: [String] = []
    private var lastFailedAudioURL: URL?

    private let startDelay: TimeInterval = 0.20
    private let previewChunkInterval: TimeInterval = 5.0
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

        // Endpoint + API key — inline dialog items. Clicking opens an NSAlert
        // with a text field; values persist via SettingsStore (UserDefaults +
        // Keychain) and the next transcription reads them with no restart.
        let endpointItem = NSMenuItem(
            title: "Endpoint: \(store.maskedBaseURL)",
            action: #selector(editEndpoint),
            keyEquivalent: ""
        )
        endpointItem.target = self
        menu.addItem(endpointItem)

        let keyItem = NSMenuItem(
            title: "API Key: \(store.maskedAPIKey)",
            action: #selector(editAPIKey),
            keyEquivalent: ""
        )
        keyItem.target = self
        menu.addItem(keyItem)
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

        let autostart = NSMenuItem(title: "Autostart", action: #selector(toggleAutostart), keyEquivalent: "")
        autostart.target = self
        autostart.state = settings.autostart ? .on : .off
        menu.addItem(autostart)

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

    @objc private func toggleAutostart() {
        settings.autostart.toggle()
        AutostartManager.setEnabled(settings.autostart)
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
        let mask = (1 << CGEventType.flagsChanged.rawValue)
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
        guard type == .flagsChanged else { return }
        let fnDown = event.flags.contains(.maskSecondaryFn)

        DispatchQueue.main.async {
            if fnDown && !self.isFnDown {
                self.isFnDown = true
                // Clear retry state if showing
                if self.lastFailedAudioURL != nil {
                    self.lastFailedAudioURL = nil
                    self.overlay.hide()
                    self.isBusy = false
                }
                self.scheduleRecordingStart()
            } else if !fnDown && self.isFnDown {
                self.isFnDown = false
                self.finishRecordingAndPaste()
            }
        }
    }

    private func scheduleRecordingStart() {
        guard !isBusy, !isRecording else { return }
        pendingStart?.cancel()
        let work = DispatchWorkItem { [weak self] in
            guard let self, self.isFnDown, !self.isRecording, !self.isBusy else { return }
            self.startRecordingSegment(resetChunks: true)
        }
        pendingStart = work
        DispatchQueue.main.asyncAfter(deadline: .now() + startDelay, execute: work)
    }

    private func startRecordingSegment(resetChunks: Bool) {
        do {
            if resetChunks {
                previewText = ""
            }
            lastPreviewChunkAt = Date()
            try recorder.start()
            isRecording = true
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
        guard isRecording else { return }
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
                    if self.isRecording { self.overlay.showPreview(self.previewText) }
                }
            } catch {
                DispatchQueue.main.async {
                    // On error, show accumulated text without suffix
                    if self.isRecording {
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

    private func finishRecordingAndPaste() {
        pendingStart?.cancel()
        pendingStart = nil

        guard isRecording else {
            overlay.hide()
            return
        }
        guard !isBusy else { return }
        guard let url = recorder.stop() else { return }

        isRecording = false
        isBusy = true
        monitorTimer?.invalidate()
        monitorTimer = nil
        overlay.showWaiting()
        print("TRANSCRIBE FINAL (full retranscription)")

        let language = settings.language
        let model = settings.selectedModel == "auto" ? nil : settings.selectedModel
        let accumulatedPreview = previewText

        DispatchQueue.global(qos: .userInitiated).async {
            do {
                // Final full retranscription of the entire recording for maximum accuracy
                var finalText = try self.transcriber.transcribe(fileURL: url, language: language, model: model)
                finalText = TextCleaner.clean(finalText)
                let cleanFinal = finalText.trimmingCharacters(in: .whitespacesAndNewlines)
                let result = cleanFinal.isEmpty ? accumulatedPreview : cleanFinal
                print("TEXT: \(result)")
                self.saveToRingBuffer(url)
                try? FileManager.default.removeItem(at: url)
                DispatchQueue.main.async {
                    self.overlay.showPreview(result)
                    self.typer.paste(result)
                    DispatchQueue.main.asyncAfter(deadline: .now() + 0.25) {
                        self.overlay.hide()
                        self.isBusy = false
                        self.previewText = ""
                    }
                }
            } catch {
                print("transcription error: \(error.localizedDescription)")
                // Save audio for retry - copy to persistent location
                let retryURL = self.saveToRingBuffer(url)
                DispatchQueue.main.async {
                    self.lastFailedAudioURL = retryURL
                    self.overlay.onRetry = { [weak self] in self?.retryTranscription() }
                    self.overlay.showRetry()
                    self.isBusy = false
                }
            }
        }
    }

    private func retryTranscription() {
        guard let url = lastFailedAudioURL else { return }
        overlay.onRetry = nil
        isBusy = true
        overlay.showWaiting()
        print("RETRY TRANSCRIPTION")

        let language = settings.language
        let model = settings.selectedModel == "auto" ? nil : settings.selectedModel

        DispatchQueue.global(qos: .userInitiated).async {
            do {
                var text = try self.transcriber.transcribe(fileURL: url, language: language, model: model)
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
                    DispatchQueue.main.asyncAfter(deadline: .now() + 0.25) {
                        self.overlay.hide()
                        self.isBusy = false
                        self.previewText = ""
                    }
                }
            } catch {
                print("retry transcription error: \(error.localizedDescription)")
                DispatchQueue.main.async {
                    self.overlay.onRetry = { [weak self] in self?.retryTranscription() }
                    self.overlay.showRetry()
                    self.isBusy = false
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
        isRecording = false
        lastFailedAudioURL = nil
        overlay.onRetry = nil
        overlay.hide()
    }


}

let app = NSApplication.shared
let delegate = VoicePasteApp()
app.delegate = delegate
app.setActivationPolicy(.accessory)
app.run()
