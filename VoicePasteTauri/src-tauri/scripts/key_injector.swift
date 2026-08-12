// key_injector.swift
//
// Helper for the blackbox test. Posts a synthetic keyboard event into the macOS
// HID event tap so the running `modifier_monitor` will see it.
//
// Usage:
//   swift key_injector.swift <keycode> <down|up>      # raw keyboard event
//   swift key_injector.swift flags <keycode> <mask>   # flagsChanged (e.g. maskAlternate=0x80000)
//
// Notes:
// - Posting to .cghidEventTap requires the calling process to have
//   Accessibility / Input Monitoring permission.
// - For the blackbox test we use a flagsChanged event for the modifier-based
//   hotkeys (RightOption etc.) and keyDown/keyUp for keycode-based hotkeys
//   (Fn/Globe/CapsLock). Both are real-world shapes the OS can deliver.

import Foundation
import CoreGraphics

func postKey(_ keyCode: Int, down: Bool) {
    guard let event = CGEvent(keyboardEventSource: nil, virtualKey: CGKeyCode(keyCode), keyDown: down) else {
        FileHandle.standardError.write(Data("failed to create event\n".utf8))
        exit(1)
    }
    event.type = down ? .keyDown : .keyUp
    event.post(tap: .cghidEventTap)
}

func postFlagsChanged(_ keyCode: Int, rawMask: UInt64) {
    guard let event = CGEvent(keyboardEventSource: nil, virtualKey: CGKeyCode(keyCode), keyDown: true) else {
        FileHandle.standardError.write(Data("failed to create event\n".utf8))
        exit(1)
    }
    event.type = .flagsChanged
    event.flags = CGEventFlags(rawValue: rawMask)
    event.post(tap: .cghidEventTap)
}

func usage() -> Never {
    FileHandle.standardError.write(Data(
        "Usage:\n  key_injector <keycode> <down|up>\n  key_injector flags <keycode> <mask>\n".utf8
    ))
    exit(1)
}

let args = CommandLine.arguments
guard args.count >= 3 else { usage() }

if args[1] == "flags" {
    guard args.count == 4,
          let keyCode = Int(args[2]),
          let mask = UInt64(args[3]) else { usage() }
    postFlagsChanged(keyCode, rawMask: mask)
} else {
    guard args.count == 3,
          let keyCode = Int(args[1]) else { usage() }
    let down: Bool
    switch args[2] {
    case "down": down = true
    case "up":   down = false
    default: usage()
    }
    postKey(keyCode, down: down)
}
