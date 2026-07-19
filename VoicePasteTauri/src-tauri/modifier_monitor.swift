#!/usr/bin/env swift
// VoicePaste Modifier Key Monitor
// Monitors modifier key events via CGEvent tap and outputs JSON lines to stdout
// Usage: ./modifier_monitor <hotkey_kind>
// hotkey_kind: fn, right_option, right_control, right_command, right_shift, caps_lock

import Foundation
import CoreGraphics
import ApplicationServices

// MARK: - Hotkey Kind
enum HotkeyKind: String {
    case fn = "fn"
    case rightOption = "right_option"
    case rightControl = "right_control"
    case rightCommand = "right_command"
    case rightShift = "right_shift"
    case capsLock = "caps_lock"
    
    var flag: CGEventFlags? {
        switch self {
        case .fn: return .maskSecondaryFn
        case .rightOption: return .maskAlternate
        case .rightControl: return .maskControl
        case .rightCommand: return .maskCommand
        case .rightShift: return .maskShift
        case .capsLock: return nil
        }
    }
    
    var keyCode: CGKeyCode? {
        switch self {
        case .capsLock: return 57
        default: return nil
        }
    }
    
    var requiresRightSide: Bool {
        switch self {
        case .rightOption, .rightControl, .rightCommand, .rightShift:
            return true
        default:
            return false
        }
    }
}

// MARK: - Event Monitor
class ModifierMonitor {
    let hotkey: HotkeyKind
    var isModifierDown = false
    var otherKeyPressedWhileDown = false
    
    init(hotkey: HotkeyKind) {
        self.hotkey = hotkey
    }
    
    func start() {
        // Check accessibility permission
        let trusted = AXIsProcessTrusted()
        if !trusted {
            output(type: "error", message: "Accessibility permission required. Grant in System Settings > Privacy & Security > Accessibility.")
            exit(1)
        }
        
        output(type: "info", message: "Event tap started for \(hotkey.rawValue)")
        
        // Create event mask for flagsChanged and key events
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
            callback: { proxy, type, event, userInfo in
                guard let userInfo = userInfo else {
                    return Unmanaged.passUnretained(event)
                }
                let monitor = Unmanaged<ModifierMonitor>.fromOpaque(userInfo).takeUnretainedValue()
                monitor.handleEvent(type: type, event: event)
                return Unmanaged.passUnretained(event)
            },
            userInfo: userInfo
        ) else {
            output(type: "error", message: "Failed to create event tap. Grant Accessibility permission.")
            exit(1)
        }
        
        let source = CFMachPortCreateRunLoopSource(kCFAllocatorDefault, tap, 0)
        CFRunLoopAddSource(CFRunLoopGetMain(), source, .commonModes)
        CGEvent.tapEnable(tap: tap, enable: true)
        
        output(type: "info", message: "Monitoring \(hotkey.rawValue) key events")
    }
    
    func handleEvent(type: CGEventType, event: CGEvent) {
        switch hotkey {
        case .capsLock:
            handleKeyCodeEvent(type: type, event: event)
        default:
            handleModifierEvent(type: type, event: event)
        }
    }
    
    func handleKeyCodeEvent(type: CGEventType, event: CGEvent) {
        guard let targetKeyCode = hotkey.keyCode else { return }
        let keyCode = event.getIntegerValueField(.keyboardEventKeycode)
        
        guard keyCode == Int64(targetKeyCode) else { return }
        
        if type == .keyDown {
            output(type: "pressed", key: hotkey.rawValue)
        } else if type == .keyUp {
            output(type: "released", key: hotkey.rawValue)
        }
    }
    
    func handleModifierEvent(type: CGEventType, event: CGEvent) {
        guard type == .flagsChanged, let flag = hotkey.flag else { return }
        
        let flags = event.flags
        let isDown = flags.contains(flag)
        
        // Check right-side requirement
        if hotkey.requiresRightSide {
            // Check if this is a right-side modifier
            // The right-side bit is encoded in the event flags
            // For now, we accept both sides but log which one
            let keyCode = event.getIntegerValueField(.keyboardEventKeycode)
            let isRightSide = isRightModifierKey(keyCode: keyCode, flag: flag)
            
            if !isRightSide {
                // Left side modifier - ignore it
                return
            }
        }
        
        if isDown && !isModifierDown {
            isModifierDown = true
            otherKeyPressedWhileDown = false
            output(type: "pressed", key: hotkey.rawValue)
        } else if !isDown && isModifierDown {
            isModifierDown = false
            if otherKeyPressedWhileDown {
                output(type: "suppressed", key: hotkey.rawValue, reason: "other_key_pressed")
            } else {
                output(type: "released", key: hotkey.rawValue)
            }
        }
    }
    
    func isRightModifierKey(keyCode: Int64, flag: CGEventFlags) -> Bool {
        // Right-side modifier key codes on macOS:
        // Right Shift: 60
        // Right Control: 62
        // Right Option: 61
        // Right Command: 54
        switch flag {
        case .maskShift:
            return keyCode == 60
        case .maskControl:
            return keyCode == 62
        case .maskAlternate:
            return keyCode == 61
        case .maskCommand:
            return keyCode == 54
        default:
            return false
        }
    }
    
    func output(type: String, key: String? = nil, message: String? = nil, reason: String? = nil) {
        var json: [String: Any] = ["type": type]
        if let key = key { json["key"] = key }
        if let message = message { json["message"] = message }
        if let reason = reason { json["reason"] = reason }
        json["timestamp"] = Int(Date().timeIntervalSince1970 * 1000)
        
        if let data = try? JSONSerialization.data(withJSONObject: json),
           let str = String(data: data, encoding: .utf8) {
            print(str)
            fflush(stdout)
        }
    }
}

// MARK: - Main
guard CommandLine.arguments.count > 1 else {
    print("Usage: modifier_monitor <hotkey_kind>")
    print("hotkey_kind: fn, right_option, right_control, right_command, right_shift, caps_lock")
    exit(1)
}

let hotkeyKind = CommandLine.arguments[1]
guard let hotkey = HotkeyKind(rawValue: hotkeyKind) else {
    print("Unknown hotkey kind: \(hotkeyKind)")
    exit(1)
}

let monitor = ModifierMonitor(hotkey: hotkey)
monitor.start()

// Run the main run loop
RunLoop.main.run()
