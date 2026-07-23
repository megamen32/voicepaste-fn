import Foundation
import CoreGraphics
@testable import ModifierMonitor

/// Construct synthetic CGEvents for unit tests.
///
/// Note: Swift's `CGEvent(keyboardEventSource:virtualKey:keyDown:)` initializer
/// actually produces an event of `type == .flagsChanged` rather than `.keyDown`/`.keyUp`
/// on some macOS versions (verified on macOS 26.6). We force the type with
/// `CGEventSetType` so tests reflect real-world event shapes.
enum EventFactory {
    /// A keyDown (down=true) or keyUp (down=false) keyboard event.
    static func key(_ keyCode: CGKeyCode, down: Bool, flags: CGEventFlags = []) -> CGEvent {
        let event = CGEvent(
            keyboardEventSource: nil,
            virtualKey: keyCode,
            keyDown: down
        )!
        event.type = down ? .keyDown : .keyUp
        event.flags = flags
        return event
    }

    /// A flagsChanged event (the type the OS fires for modifier transitions).
    static func flagsChanged(keyCode: CGKeyCode, flags: CGEventFlags) -> CGEvent {
        let event = CGEvent(
            keyboardEventSource: nil,
            virtualKey: keyCode,
            keyDown: true
        )!
        event.type = .flagsChanged
        event.flags = flags
        return event
    }
}
