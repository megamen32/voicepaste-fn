import Foundation
@testable import ModifierMonitor

/// Test helper: collects OutputEvents emitted by the core.
final class RecordingSink: OutputSink {
    private(set) var events: [OutputEvent] = []
    private let lock = NSLock()

    func send(_ event: OutputEvent) {
        lock.lock()
        events.append(event)
        lock.unlock()
    }

    /// Synchronous snapshot for assertions.
    func snapshot() -> [OutputEvent] {
        lock.lock()
        defer { lock.unlock() }
        return events
    }
}

extension OutputEvent.EventType: Equatable {
    public static func == (lhs: OutputEvent.EventType, rhs: OutputEvent.EventType) -> Bool {
        lhs.rawValue == rhs.rawValue
    }
}
