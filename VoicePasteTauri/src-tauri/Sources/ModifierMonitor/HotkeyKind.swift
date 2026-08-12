import Foundation
import CoreGraphics

/// Which physical key is being monitored. Mirrors the macOS Rust `HotkeyKind`.
public enum HotkeyKind: String, CaseIterable {
    case fn = "fn"
    case rightOption = "right_option"
    case rightControl = "right_control"
    case rightCommand = "right_command"
    case rightShift = "right_shift"
    case capsLock = "caps_lock"
    case fnControl = "fn_control"

    /// CGEventFlags bit for modifier-based keys. `nil` for keys that don't use flags
    /// (e.g. caps_lock, fn on some systems where it shows up only as keyDown/keyUp).
    public var flag: CGEventFlags? {
        switch self {
        case .fn, .fnControl: return .maskSecondaryFn
        case .rightOption: return .maskAlternate
        case .rightControl: return .maskControl
        case .rightCommand: return .maskCommand
        case .rightShift: return .maskShift
        case .capsLock: return nil
        }
    }

    /// Virtual keycode for keys that are detected via keyDown/keyUp rather than flagsChanged.
    /// We track multiple keycodes for the Fn/Globe case because Apple Silicon keyboards report
    /// different keycodes (63 = legacy Fn, 179 = Globe 🌐) depending on the model and macOS version.
    public var targetKeyCodes: Set<CGKeyCode> {
        switch self {
        case .fn, .fnControl: return [63, 179] // legacy Fn + modern Globe
        case .capsLock: return [57]
        default: return []
        }
    }

    /// Whether this hotkey requires the right-side variant of the modifier.
    public var requiresRightSide: Bool {
        switch self {
        case .rightOption, .rightControl, .rightCommand, .rightShift:
            return true
        default:
            return false
        }
    }

    /// Static lookup: does this keyCode correspond to the right-side variant of `flag`?
    public static func isRightModifierKey(keyCode: Int64, flag: CGEventFlags) -> Bool {
        switch flag {
        case .maskShift:     return keyCode == 60  // right shift
        case .maskControl:   return keyCode == 62  // right control
        case .maskAlternate: return keyCode == 61  // right option
        case .maskCommand:   return keyCode == 54  // right command
        default:             return false
        }
    }
}
