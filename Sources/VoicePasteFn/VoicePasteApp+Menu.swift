import AppKit
import AVFoundation
import ApplicationServices
import Foundation
import Security
import VoicePasteLib

extension VoicePasteApp {
    func setupMenuBar() {
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

    func rebuildMenu() {
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


}
