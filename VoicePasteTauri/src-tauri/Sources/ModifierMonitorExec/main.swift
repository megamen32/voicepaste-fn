import Foundation
import CoreGraphics
import ApplicationServices
import ModifierMonitor

// MARK: - Event tap plumbing (thin wrapper around ModifierMonitorCore)

final class TapRunner {
    let core: ModifierMonitorCore
    let sink: StdoutSink

    init(hotkey: HotkeyKind, sink: StdoutSink) {
        self.sink = sink
        self.core = ModifierMonitorCore(hotkey: hotkey, sink: sink)
    }

    func run() {
        // Accessibility is required to install the event tap.
        if !AXIsProcessTrusted() {
            sink.send(.init(
                type: .error,
                message: "Accessibility permission required. Grant in System Settings > Privacy & Security > Accessibility."
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
            options: .defaultTap,
            eventsOfInterest: CGEventMask(mask),
            callback: { _, type, event, userInfo in
                guard let userInfo = userInfo else {
                    return Unmanaged.passUnretained(event)
                }
                let runner = Unmanaged<TapRunner>.fromOpaque(userInfo).takeUnretainedValue()
                runner.core.process(event: event)
                return Unmanaged.passUnretained(event)
            },
            userInfo: userInfo
        ) else {
            sink.send(.init(
                type: .error,
                message: "Failed to create event tap. Grant Accessibility permission."
            ))
            exit(1)
        }

        let source = CFMachPortCreateRunLoopSource(kCFAllocatorDefault, tap, 0)
        CFRunLoopAddSource(CFRunLoopGetMain(), source, .commonModes)
        CGEvent.tapEnable(tap: tap, enable: true)

        sink.send(.init(type: .info, message: "Monitoring \(core.hotkey.rawValue) key events"))
        RunLoop.main.run()
    }
}

// MARK: - CLI entry

func usage() -> Never {
    let stderr = FileHandle.standardError
    stderr.write(Data("Usage: modifier_monitor <hotkey_kind>\n".utf8))
    stderr.write(Data("hotkey_kind: fn, right_option, right_control, right_command, right_shift, caps_lock\n".utf8))
    exit(1)
}

guard CommandLine.arguments.count > 1 else { usage() }
guard let kind = HotkeyKind(rawValue: CommandLine.arguments[1]) else {
    FileHandle.standardError.write(Data("Unknown hotkey kind: \(CommandLine.arguments[1])\n".utf8))
    exit(1)
}

let sink = StdoutSink()
TapRunner(hotkey: kind, sink: sink).run()
