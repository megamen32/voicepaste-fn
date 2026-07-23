import Foundation
import CoreGraphics

/// Pure event-processing core. Given a CGEvent, decides whether to emit
/// "pressed" / "released" / "suppressed" on the sink. No runloop, no event tap,
/// no accessibility check — those live in the executable target.
///
/// This split exists so the logic can be unit-tested by feeding synthetic CGEvents.
public final class ModifierMonitorCore {
    public let hotkey: HotkeyKind
    public weak var sink: OutputSink?

    /// True between a modifier-down and modifier-up event.
    public private(set) var isModifierDown = false
    /// True if a non-modifier key was pressed while `isModifierDown == true`.
    /// Reset on each new press; used to distinguish a clean release from
    /// "modifier was held while another key was used" (e.g. ⌘C while ⌘ is the hotkey).
    public private(set) var otherKeyPressedWhileDown = false

    public init(hotkey: HotkeyKind, sink: OutputSink) {
        self.hotkey = hotkey
        self.sink = sink
    }

    /// Reset state (e.g. after a stuck-key recovery).
    public func reset() {
        isModifierDown = false
        otherKeyPressedWhileDown = false
    }

    /// Process a single CGEvent. Safe to call from the event-tap callback thread.
    public func process(event: CGEvent) {
        let type = event.type
        let keyCode = event.getIntegerValueField(.keyboardEventKeycode)
        let flags = event.flags

        // 1) Keycode-style hotkeys: keyDown/keyUp for the target keycode.
        //    Used for CapsLock AND for Fn/Globe on systems where flagsChanged never fires
        //    (very common on Apple Silicon with the Globe 🌐 key — keycode 179).
        if (type == .keyDown || type == .keyUp),
           hotkey.targetKeyCodes.contains(CGKeyCode(keyCode)) {
            handleKeyCodeEvent(type: type, keyCode: keyCode)
            return
        }

        // 2) Modifier-style hotkeys: flagsChanged transitions.
        //    Used for Fn (when the OS does fire it), RightOption, RightControl, RightCommand, RightShift.
        if let flag = hotkey.flag, type == .flagsChanged {
            handleModifierFlagEvent(type: type, event: event, flag: flag, keyCode: keyCode, flags: flags)
            return
        }

        // 3) Any other keyDown while the modifier is held: remember that "another key" was used.
        //    This is the fix for the missing otherKeyPressedWhileDown update in the old code.
        if type == .keyDown && isModifierDown {
            otherKeyPressedWhileDown = true
        }
    }

    // MARK: - Modifier flag handling

    private func handleModifierFlagEvent(
        type: CGEventType,
        event: CGEvent,
        flag: CGEventFlags,
        keyCode: Int64,
        flags: CGEventFlags
    ) {
        // Only flagsChanged events carry modifier transitions.
        guard type == .flagsChanged else {
            // Other keyDown/keyUp while modifier is down → remember it for the "suppressed" branch.
            if type == .keyDown && isModifierDown {
                otherKeyPressedWhileDown = true
            }
            return
        }

        // Right-side enforcement: ignore the left-side variant.
        if hotkey.requiresRightSide,
           !HotkeyKind.isRightModifierKey(keyCode: keyCode, flag: flag) {
            return
        }

        let isDown = flags.contains(flag)

        if isDown && !isModifierDown {
            isModifierDown = true
            otherKeyPressedWhileDown = false
            emit(.init(type: .pressed, key: hotkey.rawValue))
        } else if !isDown && isModifierDown {
            isModifierDown = false
            if otherKeyPressedWhileDown {
                emit(.init(type: .suppressed, key: hotkey.rawValue, reason: "other_key_pressed"))
            } else {
                emit(.init(type: .released, key: hotkey.rawValue))
            }
        }
    }

    // MARK: - Keycode handling (Fn/Globe + CapsLock)

    private func handleKeyCodeEvent(type: CGEventType, keyCode: Int64) {
        guard type == .keyDown || type == .keyUp else { return }

        if type == .keyDown && !isModifierDown {
            isModifierDown = true
            otherKeyPressedWhileDown = false
            emit(.init(type: .pressed, key: hotkey.rawValue))
        } else if type == .keyUp && isModifierDown {
            isModifierDown = false
            if otherKeyPressedWhileDown {
                emit(.init(type: .suppressed, key: hotkey.rawValue, reason: "other_key_pressed"))
            } else {
                emit(.init(type: .released, key: hotkey.rawValue))
            }
        }
    }

    private func emit(_ event: OutputEvent) {
        sink?.send(event)
    }
}
