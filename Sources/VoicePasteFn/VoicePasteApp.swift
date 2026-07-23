import AppKit
import AVFoundation
import ApplicationServices
import Foundation
import Security
import VoicePasteLib

final class VoicePasteApp: NSObject, NSApplicationDelegate {
    let store = SettingsStore.shared
    let settings = Settings.shared
    let recorder = Recorder()
    let transcriber = Transcriber()
    let localTranscriber = LocalTranscriber()
    let typer = PasteboardTyper()
    let overlay = RecordingOverlay()

    var statusItem: NSStatusItem?
    private var eventTap: CFMachPort?

    var isFnDown = false
    let queue = RecordingQueueCoordinator()
    var pendingStart: DispatchWorkItem?
    var monitorTimer: Timer?
    var hideWorkItem: DispatchWorkItem?

    var previewText: String = ""
    var previewInFlight = false
    var lastPreviewChunkAt = Date.distantPast

    var availableModels: [String] = []
    var lastFailedAudioURL: URL?

    var startDelay: TimeInterval { settings.recordingDelay }
    var previewChunkInterval: TimeInterval { settings.realtimeChunkInterval }
    let ringBufferDir = FileManager.default.temporaryDirectory.appendingPathComponent("voicepaste-fn-ring", isDirectory: true)
    let ringBufferSize = 10

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
        let code = CGKeyCode(event.getIntegerValueField(.keyboardEventKeycode))
        if (type == .keyDown || type == .keyUp), hotkey.targetKeyCodes.contains(code) {
            isDown = type == .keyDown
        } else {
            // Modifier flag-based path. Fn uses this on older keyboards.
            guard type == .flagsChanged, let flag = hotkey.flag else { return }
            isDown = event.flags.contains(flag)
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
        } else if pressed && isFnDown {
            // Recovery for a missed key-up (sleep, focus change, or a Globe
            // event delivered through the other event family).
            isFnDown = false
            finishRecordingAndPaste()
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
        guard let url = recorder.stop() else {
            // Never leave the queue, monitor timer or indicator active when
            // AVAudioRecorder has already stopped or lost its URL.
            monitorTimer?.invalidate()
            monitorTimer = nil
            queue.reset()
            previewText = ""
            overlay.hide()
            print("recording stop recovered: recorder returned no URL")
            return
        }

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

    func stopRecordingWithoutPaste() {
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
