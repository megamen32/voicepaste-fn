import AppKit
import QuartzCore

/// A compact, language-neutral status indicator for the recording lifecycle.
/// Text is reserved for the actual transcript; recording/processing/errors are
/// communicated through motion and icons so they do not need localization.
final class RecordingOverlay {
    private typealias IndicatorState = StatusIndicatorView.State

    private var panel: NSPanel?
    private var label: NSTextField?
    private var indicator: StatusIndicatorView?
    private var clickMonitor: Any?
    var onRetry: (() -> Void)?

    func showRecording() {
        recordTestState("recording", width: 58, height: 38)
        setNonInteractive()
        showIndicator(.recording, tooltip: "Recording")
    }

    func showWaiting() {
        recordTestState("waiting", width: 58, height: 38)
        setNonInteractive()
        showIndicator(.waiting, tooltip: "Processing")
    }

    func showPreview(_ text: String) {
        let clean = text.replacingOccurrences(of: "\n", with: " ")
            .trimmingCharacters(in: .whitespacesAndNewlines)
        if clean.isEmpty {
            showRecording()
            return
        }
        setNonInteractive()
        recordTestState("preview", width: CGFloat(min(max(120, text.count + 24), 500)), height: CGFloat(text.count > 60 ? 72 : 38))
        showText(clean)
    }

    func showError(_ text: String) {
        setNonInteractive()
        showIndicator(.error, tooltip: text)
    }

    func showRetry() {
        recordTestState("retry", width: 58, height: 38)
        showIndicator(.retry, tooltip: "Retry transcription")
        DispatchQueue.main.async { [weak self] in
            self?.setInteractive()
        }
    }

    func hide() {
        DispatchQueue.main.async { [weak self] in
            guard let self else { return }
            self.stopAnimations()
            self.setNonInteractive()
            self.panel?.orderOut(nil)
            self.onRetry = nil
        }
    }

    private func showIndicator(_ state: IndicatorState, tooltip: String) {
        DispatchQueue.main.async { [weak self] in
            guard let self else { return }
            self.stopAnimations()
            self.ensurePanel(width: 58, height: 38)
            self.label?.isHidden = true
            self.indicator?.isHidden = false
            self.indicator?.state = state
            self.panel?.contentView?.toolTip = tooltip
            self.frontPanel()
        }
    }

    private func showText(_ text: String) {
        DispatchQueue.main.async { [weak self] in
            guard let self else { return }
            self.stopAnimations()
            let font = NSFont.systemFont(ofSize: 13, weight: .semibold)
            let width = min(max(120, text.width(using: font) + 24), 500)
            self.ensurePanel(width: width, height: text.count > 60 ? 72 : 38)
            self.indicator?.isHidden = true
            self.label?.isHidden = false
            self.label?.stringValue = text
            self.label?.alignment = text.count > 60 ? .left : .center
            self.label?.lineBreakMode = text.count > 60 ? .byWordWrapping : .byTruncatingTail
            self.frontPanel()
        }
    }

    private func ensurePanel(width: CGFloat, height: CGFloat) {
        let rect = NSRect(x: 0, y: 0, width: width, height: height)
        if panel == nil {
            let p = NSPanel(
                contentRect: rect,
                styleMask: [.borderless, .nonactivatingPanel],
                backing: .buffered,
                defer: false
            )
            p.isFloatingPanel = true
            p.level = .floating
            p.collectionBehavior = [.canJoinAllSpaces, .fullScreenAuxiliary, .transient, .ignoresCycle]
            p.backgroundColor = .clear
            p.isOpaque = false
            p.hasShadow = true
            p.ignoresMouseEvents = true

            let background = NSView(frame: rect)
            background.autoresizingMask = [.width, .height]
            background.wantsLayer = true
            background.layer?.backgroundColor = NSColor(calibratedWhite: 0.10, alpha: 0.97).cgColor
            background.layer?.cornerRadius = 19
            background.layer?.shadowColor = NSColor.black.cgColor
            background.layer?.shadowOpacity = 0.30
            background.layer?.shadowRadius = 12
            background.layer?.shadowOffset = .zero

            let status = StatusIndicatorView(frame: rect.insetBy(dx: 14, dy: 9))
            status.autoresizingMask = [.width, .height]
            background.addSubview(status)

            let textLabel = NSTextField(labelWithString: "")
            textLabel.frame = rect.insetBy(dx: 12, dy: 8)
            textLabel.autoresizingMask = [.width, .height]
            textLabel.font = NSFont.systemFont(ofSize: 13, weight: .semibold)
            textLabel.textColor = .white
            textLabel.backgroundColor = .clear
            textLabel.isBezeled = false
            textLabel.isEditable = false
            textLabel.isSelectable = false
            textLabel.maximumNumberOfLines = 5
            textLabel.isHidden = true
            background.addSubview(textLabel)

            p.contentView = background
            panel = p
            label = textLabel
            indicator = status
        } else {
            panel?.setContentSize(NSSize(width: width, height: height))
            indicator?.frame = rect.insetBy(dx: 14, dy: 9)
            label?.frame = rect.insetBy(dx: 12, dy: 8)
        }
    }

    private func frontPanel() {
        guard let panel else { return }
        var frame = panel.frame
        let mouse = NSEvent.mouseLocation
        let screen = NSScreen.screens.first(where: { NSMouseInRect(mouse, $0.frame, false) })
            ?? NSScreen.main
            ?? NSScreen.screens.first

        if let screen, Settings.shared.overlayCentered {
            let visible = screen.visibleFrame
            frame.origin.x = visible.midX - frame.width / 2
            frame.origin.y = visible.midY - frame.height / 2
        } else if let screen {
            frame.origin.x = mouse.x + 14
            frame.origin.y = mouse.y - frame.height - 12
            let visible = screen.visibleFrame
            if frame.maxX > visible.maxX { frame.origin.x = visible.maxX - frame.width - 8 }
            if frame.minX < visible.minX { frame.origin.x = visible.minX + 8 }
            if frame.minY < visible.minY { frame.origin.y = mouse.y + 20 }
            if frame.maxY > visible.maxY { frame.origin.y = visible.maxY - frame.height - 8 }
        }
        panel.setFrame(frame, display: true, animate: true)
        panel.orderFrontRegardless()
    }

    private func setInteractive() {
        panel?.ignoresMouseEvents = false
        clickMonitor = NSEvent.addLocalMonitorForEvents(matching: .leftMouseDown) { [weak self] event in
            guard let self, let panel = self.panel else { return event }
            if panel.frame.contains(NSEvent.mouseLocation) {
                self.onRetry?()
                return nil
            }
            return event
        }
    }

    private func setNonInteractive() {
        if let clickMonitor {
            NSEvent.removeMonitor(clickMonitor)
            self.clickMonitor = nil
        }
        panel?.ignoresMouseEvents = true
    }

    private func stopAnimations() {
        indicator?.stopAnimations()
    }

    private func recordTestState(_ state: String, width: CGFloat, height: CGFloat) {
        guard let path = ProcessInfo.processInfo.environment["VOICEPASTE_TEST_OVERLAY_LOG"] else { return }
        let line = "\(state) \(Int(width))x\(Int(height))\n"
        guard let data = line.data(using: .utf8) else { return }
        if FileManager.default.fileExists(atPath: path),
           let handle = try? FileHandle(forWritingTo: URL(fileURLWithPath: path)) {
            handle.seekToEndOfFile()
            handle.write(data)
            try? handle.close()
        } else {
            FileManager.default.createFile(atPath: path, contents: data)
        }
    }
}

private final class StatusIndicatorView: NSView {
    enum State {
        case recording
        case waiting
        case error
        case retry
    }

    var state: State = .recording { didSet { updateState() } }
    private let dot = CALayer()
    private let spinner = NSProgressIndicator()
    private let icon = NSImageView()

    override init(frame frameRect: NSRect) {
        super.init(frame: frameRect)
        wantsLayer = true
        dot.cornerRadius = 7
        layer?.addSublayer(dot)

        spinner.style = .spinning
        spinner.controlSize = .small
        spinner.isDisplayedWhenStopped = false
        addSubview(spinner)

        icon.imageScaling = .scaleProportionallyUpOrDown
        icon.contentTintColor = .white
        addSubview(icon)
    }

    required init?(coder: NSCoder) { fatalError("init(coder:) has not been implemented") }

    override func layout() {
        super.layout()
        dot.frame = NSRect(x: bounds.midX - 7, y: bounds.midY - 7, width: 14, height: 14)
        spinner.frame = NSRect(x: bounds.midX - 10, y: bounds.midY - 10, width: 20, height: 20)
        icon.frame = NSRect(x: bounds.midX - 9, y: bounds.midY - 9, width: 18, height: 18)
    }

    func stopAnimations() {
        dot.removeAllAnimations()
        spinner.stopAnimation(nil)
        spinner.isHidden = true
        icon.isHidden = true
        dot.isHidden = true
    }

    private func updateState() {
        stopAnimations()
        switch state {
        case .recording:
            dot.isHidden = false
            dot.backgroundColor = NSColor.systemRed.cgColor
            let pulse = CABasicAnimation(keyPath: "opacity")
            pulse.fromValue = 0.35
            pulse.toValue = 1.0
            pulse.duration = 0.8
            pulse.autoreverses = true
            pulse.repeatCount = .infinity
            dot.add(pulse, forKey: "recording-pulse")
        case .waiting:
            spinner.isHidden = false
            spinner.startAnimation(nil)
        case .error:
            showIcon("exclamationmark.triangle.fill", tint: .systemOrange)
        case .retry:
            showIcon("arrow.clockwise", tint: .systemOrange)
        }
    }

    private func showIcon(_ name: String, tint: NSColor) {
        icon.image = NSImage(systemSymbolName: name, accessibilityDescription: nil)
        icon.contentTintColor = tint
        icon.isHidden = false
    }
}

private extension String {
    func width(using font: NSFont) -> CGFloat {
        (self as NSString).size(withAttributes: [.font: font]).width
    }
}
