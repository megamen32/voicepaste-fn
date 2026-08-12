import Foundation

/// JSON line event emitted by the modifier monitor on stdout.
/// The Rust side (hotkey.rs) parses these and forwards to Tauri's event bus.
public struct OutputEvent: Equatable {
    public enum EventType: String {
        case info
        case error
        case pressed
        case released
        case suppressed
    }

    public let type: EventType
    public let key: String?
    public let message: String?
    public let reason: String?
    public let timestamp: Int64

    public init(
        type: EventType,
        key: String? = nil,
        message: String? = nil,
        reason: String? = nil,
        timestamp: Int64 = Int64(Date().timeIntervalSince1970 * 1000)
    ) {
        self.type = type
        self.key = key
        self.message = message
        self.reason = reason
        self.timestamp = timestamp
    }

    /// JSON object suitable for JSONSerialization.
    public var jsonObject: [String: Any] {
        var json: [String: Any] = ["type": type.rawValue, "timestamp": timestamp]
        if let key = key { json["key"] = key }
        if let message = message { json["message"] = message }
        if let reason = reason { json["reason"] = reason }
        return json
    }

    /// Render to a single JSON line (no trailing newline).
    public func toJSONLine() -> String {
        guard
            let data = try? JSONSerialization.data(withJSONObject: jsonObject, options: []),
            let str = String(data: data, encoding: .utf8)
        else {
            return "{\"type\":\"error\",\"message\":\"encode_failed\"}"
        }
        return str
    }
}

/// Anything that can receive an OutputEvent. Production code uses StdoutSink,
/// tests use RecordingSink to capture events for assertions.
public protocol OutputSink: AnyObject {
    func send(_ event: OutputEvent)
}

/// Writes one JSON line per event to a file handle (defaults to stdout).
public final class StdoutSink: OutputSink {
    private let handle: FileHandle
    private let lock = NSLock()

    public init(handle: FileHandle = FileHandle.standardOutput) {
        self.handle = handle
    }

    public func send(_ event: OutputEvent) {
        let line = event.toJSONLine() + "\n"
        guard let data = line.data(using: .utf8) else { return }
        lock.lock()
        defer { lock.unlock() }
        handle.write(data)
    }
}
