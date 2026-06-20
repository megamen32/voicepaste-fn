import AppKit
import AVFoundation
import Foundation

// MARK: - Defaults / Settings

final class Config {
    let baseURL: URL
    let apiKey: String
    let model: String

    init() {
        let base = ProcessInfo.processInfo.environment["OPENAI_BASE_URL"] ?? "https://example.com/v1"
        self.baseURL = URL(string: base.trimmingCharacters(in: CharacterSet(charactersIn: "/")))!
        self.apiKey = ProcessInfo.processInfo.environment["OPENAI_API_KEY"] ?? "***"
        self.model = ProcessInfo.processInfo.environment["TRANSCRIBE_MODEL"] ?? "whisper-1"
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
    let config: Config

    init(config: Config) {
        self.config = config
    }

    func transcribe(fileURL: URL, language: Language) throws -> String {
        var request = URLRequest(url: config.baseURL.appendingPathComponent("audio/transcriptions"))
        request.httpMethod = "POST"
        if !config.apiKey.isEmpty {
            request.setValue("Bearer \(config.apiKey)", forHTTPHeaderField: "Authorization")
        }

        let boundary = "Boundary-\(UUID().uuidString)"
        request.setValue("multipart/form-data; boundary=\(boundary)", forHTTPHeaderField: "Content-Type")

        var body = Data()
        appendField(name: "model", value: config.model, boundary: boundary, body: &body)
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
        return (json?["text"] as? String)?.trimmingCharacters(in: .whitespacesAndNewlines) ?? ""
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
}

extension Data {
    mutating func appendString(_ string: String) {
        append(string.data(using: .utf8)!)
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

    func showRecording() {
        show(text: "● REC", maxWidth: 110)
    }

    func showWaiting() {
        show(text: "…", maxWidth: 64)
    }

    func showPreview(_ text: String) {
        let clean = text.replacingOccurrences(of: "\n", with: " ").trimmingCharacters(in: .whitespacesAndNewlines)
        if clean.isEmpty {
            showRecording()
        } else {
            let limited = String(clean.prefix(140))
            show(text: limited, maxWidth: 420)
        }
    }

    func showError(_ text: String) {
        show(text: "ERR: \(String(text.prefix(80)))", maxWidth: 420)
    }

    func hide() {
        DispatchQueue.main.async { self.panel?.orderOut(nil) }
    }

    private func show(text: String, maxWidth: CGFloat) {
        DispatchQueue.main.async { self.showOnMain(text: text, maxWidth: maxWidth) }
    }

    private func showOnMain(text: String, maxWidth: CGFloat) {
        let font = NSFont.systemFont(ofSize: 13, weight: .semibold)
        let textWidth = ceil((text as NSString).size(withAttributes: [.font: font]).width) + 26
        let width = min(max(textWidth, 64), maxWidth)
        let height: CGFloat = 38

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
            l.alignment = .center
            l.lineBreakMode = .byTruncatingTail
            l.font = font
            l.textColor = .white
            l.backgroundColor = .clear
            l.isBezeled = false
            l.isEditable = false
            l.isSelectable = false

            visual.addSubview(l)
            p.contentView = visual
            panel = p
            label = l
        }

        label?.font = font
        label?.stringValue = text

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

        p.setFrame(frame, display: true)
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

        if enabled {
            let cwd = URL(fileURLWithPath: fm.currentDirectoryPath)
            let run = cwd.appendingPathComponent("run.sh").path
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
    private let config = Config()
    private let settings = Settings.shared
    private let recorder = Recorder()
    private lazy var transcriber = Transcriber(config: config)
    private let typer = PasteboardTyper()
    private let overlay = RecordingOverlay()

    private var statusItem: NSStatusItem?
    private var eventTap: CFMachPort?

    private var isFnDown = false
    private var isRecording = false
    private var isBusy = false
    private var pendingStart: DispatchWorkItem?
    private var monitorTimer: Timer?

    private var committedChunks: [String] = []
    private var speechSeenInSegment = false
    private var silenceStartedAt: Date?
    private var previewInFlight = false
    private var lastPreviewAt = Date.distantPast

    private let startDelay: TimeInterval = 0.20
    private let speechThresholdDb: Float = -42
    private let silenceCommitDelay: TimeInterval = 0.90
    private let minSegmentDuration: TimeInterval = 0.55
    private let previewEvery: TimeInterval = 5.0

    func applicationDidFinishLaunching(_ notification: Notification) {
        NSApp.setActivationPolicy(.accessory)
        requestMicrophonePermission()
        setupMenuBar()
        installEventTap()

        print("VoicePasteFn started")
        print("Endpoint: \(config.baseURL.absoluteString)")
        print("Model: \(config.model)")
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

        let realtime = NSMenuItem(title: "Realtime preview", action: #selector(toggleRealtime), keyEquivalent: "")
        realtime.target = self
        realtime.state = settings.realtimePreview ? .on : .off
        menu.addItem(realtime)

        let autostart = NSMenuItem(title: "Autostart", action: #selector(toggleAutostart), keyEquivalent: "")
        autostart.target = self
        autostart.state = settings.autostart ? .on : .off
        menu.addItem(autostart)

        menu.addItem(.separator())
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

    @objc private func toggleRealtime() {
        settings.realtimePreview.toggle()
        rebuildMenu()
    }

    @objc private func toggleAutostart() {
        settings.autostart.toggle()
        AutostartManager.setEnabled(settings.autostart)
        rebuildMenu()
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
                committedChunks = []
            }
            speechSeenInSegment = false
            silenceStartedAt = nil
            lastPreviewAt = Date()
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

        let power = recorder.averagePower()
        let now = Date()
        let duration = recorder.currentTime
        let speaking = power > speechThresholdDb

        if speaking {
            speechSeenInSegment = true
            silenceStartedAt = nil
        } else if speechSeenInSegment && silenceStartedAt == nil {
            silenceStartedAt = now
        }

        if settings.realtimePreview && duration >= minSegmentDuration {
            if now.timeIntervalSince(lastPreviewAt) >= previewEvery {
                lastPreviewAt = now
                sendPreviewForCurrentSegment()
            }

            if let silenceStartedAt,
               now.timeIntervalSince(silenceStartedAt) >= silenceCommitDelay,
               speechSeenInSegment {
                commitCurrentSegmentAndContinue()
            }
        }
    }

    private func sendPreviewForCurrentSegment() {
        guard settings.realtimePreview, !previewInFlight, let url = recorder.currentURL else { return }
        previewInFlight = true
        overlay.showWaiting()

        let previewURL = FileManager.default.temporaryDirectory
            .appendingPathComponent("voicepaste-preview-\(UUID().uuidString)")
            .appendingPathExtension("wav")
        try? FileManager.default.copyItem(at: url, to: previewURL)

        DispatchQueue.global(qos: .utility).async {
            defer {
                try? FileManager.default.removeItem(at: previewURL)
                DispatchQueue.main.async { self.previewInFlight = false }
            }
            do {
                let text = try self.transcriber.transcribe(fileURL: previewURL, language: self.settings.language)
                DispatchQueue.main.async {
                    if self.isRecording { self.overlay.showPreview(text) }
                }
            } catch {
                DispatchQueue.main.async {
                    if self.isRecording { self.overlay.showRecording() }
                }
            }
        }
    }

    private func commitCurrentSegmentAndContinue() {
        guard isRecording else { return }
        guard let url = recorder.stop() else { return }
        isRecording = false
        monitorTimer?.invalidate()
        monitorTimer = nil
        overlay.showWaiting()

        let language = settings.language
        DispatchQueue.global(qos: .userInitiated).async {
            defer { try? FileManager.default.removeItem(at: url) }
            do {
                let text = try self.transcriber.transcribe(fileURL: url, language: language)
                DispatchQueue.main.async {
                    let clean = text.trimmingCharacters(in: .whitespacesAndNewlines)
                    if !clean.isEmpty { self.committedChunks.append(clean) }
                    if self.isFnDown {
                        self.startRecordingSegment(resetChunks: false)
                    }
                }
            } catch {
                DispatchQueue.main.async {
                    print("chunk transcription error: \(error.localizedDescription)")
                    if self.isFnDown {
                        self.startRecordingSegment(resetChunks: false)
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
        print("TRANSCRIBE FINAL")

        let language = settings.language
        let chunksBeforeFinal = committedChunks

        DispatchQueue.global(qos: .userInitiated).async {
            defer { try? FileManager.default.removeItem(at: url) }

            var chunks = chunksBeforeFinal
            do {
                let finalText = try self.transcriber.transcribe(fileURL: url, language: language)
                let cleanFinal = finalText.trimmingCharacters(in: .whitespacesAndNewlines)
                if !cleanFinal.isEmpty { chunks.append(cleanFinal) }

                let joined = self.joinChunks(chunks)
                print("TEXT: \(joined)")
                DispatchQueue.main.async {
                    self.overlay.showPreview(joined)
                    self.typer.paste(joined)
                    DispatchQueue.main.asyncAfter(deadline: .now() + 0.25) {
                        self.overlay.hide()
                        self.isBusy = false
                        self.committedChunks = []
                    }
                }
            } catch {
                print("transcription error: \(error.localizedDescription)")
                DispatchQueue.main.async {
                    self.overlay.showError(error.localizedDescription)
                    DispatchQueue.main.asyncAfter(deadline: .now() + 1.0) {
                        self.overlay.hide()
                        self.isBusy = false
                        self.committedChunks = []
                    }
                }
            }
        }
    }

    private func stopRecordingWithoutPaste() {
        pendingStart?.cancel()
        pendingStart = nil
        monitorTimer?.invalidate()
        monitorTimer = nil
        recorder.stopWithoutReturning()
        isRecording = false
        overlay.hide()
    }

    private func joinChunks(_ chunks: [String]) -> String {
        chunks
            .map { $0.trimmingCharacters(in: .whitespacesAndNewlines) }
            .filter { !$0.isEmpty }
            .joined(separator: " ")
            .replacingOccurrences(of: "  ", with: " ")
    }
}

let app = NSApplication.shared
let delegate = VoicePasteApp()
app.delegate = delegate
app.setActivationPolicy(.accessory)
app.run()
