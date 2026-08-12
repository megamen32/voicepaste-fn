import XCTest
import CoreGraphics
@testable import ModifierMonitor

/// TDD: these tests describe the expected behavior. They are written BEFORE the
/// fix and exercise the three real-world bugs that made Fn/other keys unreliable:
///
///   1. Fn/Globe on Apple Silicon — flagsChanged is often NEVER fired, only keyDown/keyUp for keycode 63/179.
///   2. otherKeyPressedWhileDown was never updated, so modifier+key combo never produced "suppressed".
///   3. Right-side detection must ignore left-side modifier events.
final class ModifierMonitorCoreTests: XCTestCase {

    // MARK: - Bug #1: Fn/Globe must work via keyDown/keyUp, not just flagsChanged

    /// On macOS keyboards that report Fn as keycode 63 (legacy), keyDown/keyUp is the
    /// only reliable signal. The old code only watched flagsChanged → never fired.
    func test_fnKeyDown_keycode63_emitsPressed() {
        let sink = RecordingSink()
        let core = ModifierMonitorCore(hotkey: .fn, sink: sink)

        core.process(event: EventFactory.key(63, down: true))

        let events = sink.snapshot()
        XCTAssertEqual(events.count, 1, "expected exactly one event, got \(events)")
        XCTAssertEqual(events.first?.type, .pressed)
        XCTAssertEqual(events.first?.key, "fn")
    }

    /// Apple Silicon Macs with the Globe 🌐 key report keycode 179.
    func test_globeKeyDown_keycode179_emitsPressed() {
        let sink = RecordingSink()
        let core = ModifierMonitorCore(hotkey: .fn, sink: sink)

        core.process(event: EventFactory.key(179, down: true))

        XCTAssertEqual(sink.snapshot().first?.type, .pressed)
    }

    /// Press and release Fn cleanly (no other key) → "released".
    func test_fnKeyUp_afterCleanHold_emitsReleased() {
        let sink = RecordingSink()
        let core = ModifierMonitorCore(hotkey: .fn, sink: sink)

        core.process(event: EventFactory.key(63, down: true))
        core.process(event: EventFactory.key(63, down: false))

        let events = sink.snapshot()
        XCTAssertEqual(events.map(\.type), [.pressed, .released])
    }

    // MARK: - Bug #2: modifier + another key must produce "suppressed"

    /// Press Fn, press A, release A, release Fn → the trailing release must be
    /// "suppressed", not "released". The old code never set otherKeyPressedWhileDown.
    func test_fnPlusLetter_emitsSuppressedOnRelease() {
        let sink = RecordingSink()
        let core = ModifierMonitorCore(hotkey: .fn, sink: sink)

        core.process(event: EventFactory.key(63, down: true))   // Fn down
        core.process(event: EventFactory.key(0, down: true))    // A down
        core.process(event: EventFactory.key(0, down: false))   // A up
        core.process(event: EventFactory.key(63, down: false))  // Fn up

        let types = sink.snapshot().map(\.type)
        XCTAssertEqual(types, [.pressed, .suppressed])
    }

    /// Same scenario for RightOption: ⌥+C must not trigger "released" on ⌥ release.
    func test_rightOptionPlusLetter_emitsSuppressedOnRelease() {
        let sink = RecordingSink()
        let core = ModifierMonitorCore(hotkey: .rightOption, sink: sink)

        // Right option down (keyCode 61, flag set)
        core.process(event: EventFactory.flagsChanged(keyCode: 61, flags: .maskAlternate))
        // Press C (keyCode 8)
        core.process(event: EventFactory.key(8, down: true))
        core.process(event: EventFactory.key(8, down: false))
        // Right option up
        core.process(event: EventFactory.flagsChanged(keyCode: 61, flags: []))

        let types = sink.snapshot().map(\.type)
        XCTAssertEqual(types, [.pressed, .suppressed])
    }

    // MARK: - Bug #3: right-side enforcement

    /// Left option (keyCode 58) must be ignored when right option is the hotkey.
    func test_leftOption_isIgnoredForRightOptionHotkey() {
        let sink = RecordingSink()
        let core = ModifierMonitorCore(hotkey: .rightOption, sink: sink)

        core.process(event: EventFactory.flagsChanged(keyCode: 58, flags: .maskAlternate))
        core.process(event: EventFactory.flagsChanged(keyCode: 58, flags: []))

        XCTAssertTrue(sink.snapshot().isEmpty, "left option must not trigger right-option hotkey")
    }

    /// Right option (keyCode 61) is accepted.
    func test_rightOption_isAcceptedForRightOptionHotkey() {
        let sink = RecordingSink()
        let core = ModifierMonitorCore(hotkey: .rightOption, sink: sink)

        core.process(event: EventFactory.flagsChanged(keyCode: 61, flags: .maskAlternate))
        core.process(event: EventFactory.flagsChanged(keyCode: 61, flags: []))

        let types = sink.snapshot().map(\.type)
        XCTAssertEqual(types, [.pressed, .released])
    }

    // MARK: - CapsLock via keycode

    func test_capsLock_keyDownUp_emitsPressedAndReleased() {
        let sink = RecordingSink()
        let core = ModifierMonitorCore(hotkey: .capsLock, sink: sink)

        core.process(event: EventFactory.key(57, down: true))
        core.process(event: EventFactory.key(57, down: false))

        XCTAssertEqual(sink.snapshot().map(\.type), [.pressed, .released])
    }

    // MARK: - Fn via flagsChanged (when the OS does fire it)

    /// Some macOS versions DO fire flagsChanged for Fn. Should also work.
    func test_fnViaFlagsChanged_emitsPressedAndReleased() {
        let sink = RecordingSink()
        let core = ModifierMonitorCore(hotkey: .fn, sink: sink)

        core.process(event: EventFactory.flagsChanged(keyCode: 63, flags: .maskSecondaryFn))
        core.process(event: EventFactory.flagsChanged(keyCode: 63, flags: []))

        XCTAssertEqual(sink.snapshot().map(\.type), [.pressed, .released])
    }

    func test_fnControl_requiresControlAndEmitsCleanHold() {
        let sink = RecordingSink()
        let core = ModifierMonitorCore(hotkey: .fnControl, sink: sink)

        core.process(event: EventFactory.key(63, down: true))
        XCTAssertTrue(sink.snapshot().isEmpty, "bare Fn must not start the automation")

        core.process(event: EventFactory.key(63, down: true, flags: [.maskControl]))
        core.process(event: EventFactory.key(63, down: false, flags: [.maskControl]))

        XCTAssertEqual(sink.snapshot().map(\.type), [.pressed, .released])
        XCTAssertEqual(sink.snapshot().first?.key, "fn_control")
    }

    func test_fnWithControlDoesNotStartTheNormalFnHotkey() {
        let sink = RecordingSink()
        let core = ModifierMonitorCore(hotkey: .fn, sink: sink)

        core.process(event: EventFactory.key(63, down: true, flags: [.maskControl]))
        core.process(event: EventFactory.key(63, down: false, flags: [.maskControl]))

        XCTAssertTrue(sink.snapshot().isEmpty, "Fn+Control belongs to automation, not regular dictation")
    }

    func test_normalFnStaysSilentWhenControlIsReleasedBeforeFn() {
        let sink = RecordingSink()
        let core = ModifierMonitorCore(hotkey: .fn, sink: sink)

        core.process(event: EventFactory.key(63, down: true, flags: [.maskControl]))
        // The user releases Control first; the remaining Fn flag must not be
        // mistaken for a new ordinary Fn press.
        core.process(event: EventFactory.flagsChanged(keyCode: 59, flags: [.maskSecondaryFn]))
        core.process(event: EventFactory.key(63, down: false))

        XCTAssertTrue(sink.snapshot().isEmpty)
    }

    func test_normalFnRecoversAfterFnControlChord() {
        let sink = RecordingSink()
        let core = ModifierMonitorCore(hotkey: .fn, sink: sink)

        core.process(event: EventFactory.key(63, down: true))
        core.process(event: EventFactory.flagsChanged(keyCode: 59, flags: [.maskSecondaryFn, .maskControl]))
        core.process(event: EventFactory.flagsChanged(keyCode: 59, flags: [.maskSecondaryFn]))
        core.process(event: EventFactory.key(63, down: false))
        core.process(event: EventFactory.key(63, down: true))
        core.process(event: EventFactory.key(63, down: false))

        // The first press is intentionally handed over to the automation
        // route by Rust; after the chord ends, bare Fn must work normally.
        XCTAssertEqual(sink.snapshot().map(\.type), [.pressed, .pressed, .released])
    }

    // MARK: - State hygiene

    /// After "suppressed", a fresh press must reset otherKeyPressedWhileDown.
    /// Otherwise a single accidental keypress would poison the rest of the session.
    func test_stateResetsAfterSuppressed() {
        let sink = RecordingSink()
        let core = ModifierMonitorCore(hotkey: .fn, sink: sink)

        // First press: Fn + A → suppressed
        core.process(event: EventFactory.key(63, down: true))
        core.process(event: EventFactory.key(0, down: true))
        core.process(event: EventFactory.key(0, down: false))
        core.process(event: EventFactory.key(63, down: false))

        // Second press: Fn alone → must emit "released", not "suppressed"
        core.process(event: EventFactory.key(63, down: true))
        core.process(event: EventFactory.key(63, down: false))

        let types = sink.snapshot().map(\.type)
        XCTAssertEqual(types, [.pressed, .suppressed, .pressed, .released])
    }
}
