import Foundation
import CoreGraphics
import ApplicationServices
import AppKit
import AVFoundation
import Speech
import ModifierMonitor

// MARK: - Event tap plumbing (thin wrapper around ModifierMonitorCore)

final class TapRunner {
    let core: ModifierMonitorCore
    let sink: StdoutSink
    private var tap: CFMachPort?

    init(hotkey: HotkeyKind, sink: StdoutSink) {
        self.sink = sink
        self.core = ModifierMonitorCore(hotkey: hotkey, sink: sink)
    }

    func run() {
        // A listen-only tap deliberately does not modify key events. macOS
        // protects it through Input Monitoring; Accessibility is still needed
        // separately by VoicePaste to paste the finished transcription.
        if !CGPreflightListenEventAccess() {
            sink.send(.init(
                type: .error,
                message: "Input Monitoring permission required. Grant it in System Settings > Privacy & Security > Input Monitoring."
            ))
            exit(1)
        }

        sink.send(.init(type: .info, message: "Event tap started for \(core.hotkey.rawValue)"))

        // flagsChanged + keyDown + keyUp so we cover both modifier-style and keycode-style hotkeys.
        var mask: UInt64 = 0
        mask |= (1 << CGEventType.flagsChanged.rawValue)
        mask |= (1 << CGEventType.keyDown.rawValue)
        mask |= (1 << CGEventType.keyUp.rawValue)

        let userInfo = Unmanaged.passUnretained(self).toOpaque()

        guard let tap = CGEvent.tapCreate(
            tap: .cgSessionEventTap,
            place: .headInsertEventTap,
            options: .listenOnly,
            eventsOfInterest: CGEventMask(mask),
            callback: { _, type, event, userInfo in
                guard let userInfo = userInfo else {
                    return Unmanaged.passUnretained(event)
                }
                let runner = Unmanaged<TapRunner>.fromOpaque(userInfo).takeUnretainedValue()
                if type == .tapDisabledByTimeout || type == .tapDisabledByUserInput {
                    runner.reenableTap(reason: type == .tapDisabledByTimeout ? "timeout" : "user_input")
                    return Unmanaged.passUnretained(event)
                }
                runner.core.process(event: event)
                return Unmanaged.passUnretained(event)
            },
            userInfo: userInfo
        ) else {
            sink.send(.init(
                type: .error,
                message: "Failed to create the Input Monitoring event tap. Check the permission and restart VoicePaste."
            ))
            exit(1)
        }
        self.tap = tap

        let source = CFMachPortCreateRunLoopSource(kCFAllocatorDefault, tap, 0)
        CFRunLoopAddSource(CFRunLoopGetMain(), source, .commonModes)
        CGEvent.tapEnable(tap: tap, enable: true)

        sink.send(.init(type: .info, message: "Monitoring \(core.hotkey.rawValue) key events"))
        RunLoop.main.run()
    }

    private func reenableTap(reason: String) {
        guard let tap else { return }
        CGEvent.tapEnable(tap: tap, enable: true)
        sink.send(.init(type: .info, message: "Event tap re-enabled after \(reason)"))
    }
}

// MARK: - CLI entry

private func printPermissions() {
    let values: [String: Bool] = [
        "microphone": AVCaptureDevice.authorizationStatus(for: .audio) == .authorized,
        "speech_recognition": SFSpeechRecognizer.authorizationStatus() == .authorized,
        "accessibility": AXIsProcessTrusted(),
        "input_monitoring": CGPreflightListenEventAccess(),
    ]
    let data = try! JSONSerialization.data(withJSONObject: values, options: [.sortedKeys])
    print(String(decoding: data, as: UTF8.self))
}

private func waitFor(_ start: (@escaping (Bool) -> Void) -> Void) {
    let semaphore = DispatchSemaphore(value: 0)
    start { _ in semaphore.signal() }
    _ = semaphore.wait(timeout: .now() + 90)
}

private func requestPermissions(includeSpeech: Bool) {
    if AVCaptureDevice.authorizationStatus(for: .audio) == .notDetermined {
        waitFor { complete in
            AVCaptureDevice.requestAccess(for: .audio, completionHandler: complete)
        }
    }

    if includeSpeech && SFSpeechRecognizer.authorizationStatus() == .notDetermined {
        waitFor { complete in
            SFSpeechRecognizer.requestAuthorization { status in
                complete(status == .authorized)
            }
        }
    }

    if !CGPreflightListenEventAccess() {
        _ = CGRequestListenEventAccess()
    }

    if !AXIsProcessTrusted() {
        let options = [kAXTrustedCheckOptionPrompt.takeUnretainedValue() as String: true] as CFDictionary
        _ = AXIsProcessTrustedWithOptions(options)
    }

    printPermissions()
}

/// The Rust app sends the completed transcript on stdin. Keep this in the
/// signed macOS helper so writing the pasteboard and posting Cmd+V share the
/// same trusted process, exactly like the working Swift VoicePaste client.
private func pasteFromStandardInput(targetPID: pid_t?) -> Never {
    let data = FileHandle.standardInput.readDataToEndOfFile()
    guard let text = String(data: data, encoding: .utf8)?
        .trimmingCharacters(in: .whitespacesAndNewlines), !text.isEmpty else {
        FileHandle.standardError.write(Data("No text supplied for paste\n".utf8))
        exit(2)
    }

    let pasteboard = NSPasteboard.general
    pasteboard.clearContents()
    guard pasteboard.setString(text, forType: .string) else {
        FileHandle.standardError.write(Data("Failed to write the macOS pasteboard\n".utf8))
        exit(3)
    }

    guard AXIsProcessTrusted() else {
        FileHandle.standardError.write(Data("Accessibility permission is required to paste with Cmd+V\n".utf8))
        exit(5)
    }

    // The overlay may have become key while transcription ran. Return focus
    // to the app that owned it when recording began before injecting Cmd+V.
    if let targetPID, let target = NSRunningApplication(processIdentifier: targetPID) {
        _ = target.activate(options: [.activateIgnoringOtherApps])
    }

    usleep(80_000)
    let source = CGEventSource(stateID: .combinedSessionState)
    guard let keyDown = CGEvent(keyboardEventSource: source, virtualKey: 0x09, keyDown: true),
          let keyUp = CGEvent(keyboardEventSource: source, virtualKey: 0x09, keyDown: false) else {
        FileHandle.standardError.write(Data("Failed to create Cmd+V events\n".utf8))
        exit(4)
    }
    keyDown.flags = .maskCommand
    keyUp.flags = .maskCommand
    keyDown.post(tap: .cghidEventTap)
    keyUp.post(tap: .cghidEventTap)
    exit(0)
}

func usage() -> Never {
    let stderr = FileHandle.standardError
    stderr.write(Data("Usage: modifier_monitor <hotkey_kind>\n".utf8))
    stderr.write(Data("hotkey_kind: fn, right_option, right_control, right_command, right_shift, caps_lock\n".utf8))
    exit(1)
}

guard CommandLine.arguments.count > 1 else { usage() }

let arguments = Array(CommandLine.arguments.dropFirst())
if arguments.first == "--permissions" {
    printPermissions()
    exit(0)
}
if arguments.first == "--request-permissions" {
    requestPermissions(includeSpeech: arguments.contains("--include-speech"))
    exit(0)
}
if arguments.first == "--paste" {
    let targetPID: pid_t?
    if arguments.count >= 3, arguments[1] == "--pid", let rawPID = Int32(arguments[2]) {
        targetPID = pid_t(rawPID)
    } else {
        targetPID = arguments.dropFirst().first.flatMap { value in
            Int32(value).map { pid_t($0) }
        }
    }
    pasteFromStandardInput(targetPID: targetPID)
}

guard let kind = HotkeyKind(rawValue: CommandLine.arguments[1]) else {
    FileHandle.standardError.write(Data("Unknown hotkey kind: \(CommandLine.arguments[1])\n".utf8))
    exit(1)
}

let sink = StdoutSink()
TapRunner(hotkey: kind, sink: sink).run()
