import CoreGraphics
import Foundation

let needle = (CommandLine.arguments.dropFirst().first ?? "VoicePaste").lowercased()
let windows = (CGWindowListCopyWindowInfo(.optionOnScreenOnly, kCGNullWindowID) as? [[String: Any]]) ?? []
let result: [[String: Any]] = windows.compactMap { window in
    let owner = (window[kCGWindowOwnerName as String] as? String) ?? ""
    guard owner.lowercased().contains(needle) else { return nil }
    let bounds = window[kCGWindowBounds as String] as? [String: Any]
    return [
        "owner": owner,
        "layer": window[kCGWindowLayer as String] as? Int ?? -1,
        "onscreen": window[kCGWindowIsOnscreen as String] as? Bool ?? false,
        "bounds": [
            "width": bounds?["Width"] as? Double ?? 0,
            "height": bounds?["Height"] as? Double ?? 0,
        ],
    ]
}
let data = try! JSONSerialization.data(withJSONObject: result, options: [.sortedKeys])
print(String(decoding: data, as: UTF8.self))
