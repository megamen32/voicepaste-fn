import Foundation

/// Actions the coordinator tells the caller to perform.
public enum RecordingAction {
    case startRecording      // Begin a new recording segment
    case showWaiting         // Show the "waiting" overlay
    case doNothing           // No action needed
}

/// Pure state machine for the recording queue.
///
/// Manages the lifecycle: recording → transcribing → (optional queued recording).
/// When a new recording is requested while transcription is in progress,
/// it is queued and automatically started when transcription completes.
public final class RecordingQueueCoordinator {

    /// Whether audio is currently being recorded.
    public var isRecording: Bool = false

    /// Whether transcription (or post-paste delay) is currently in progress.
    public private(set) var isBusy: Bool = false

    /// Whether a new recording has been queued for when busy completes.
    public private(set) var hasPendingRecording: Bool = false

    public init() {}

    // MARK: - Recording lifecycle

    /// Called when the recorder actually starts capturing audio.
    public func onRecordingStarted() {
        isRecording = true
    }

    /// Called when the recorder stops and audio is ready for transcription.
    /// Returns `.showWaiting` to indicate the overlay should show "processing".
    @discardableResult
    public func onRecordingStopped() -> RecordingAction {
        isRecording = false
        isBusy = true
        return .showWaiting
    }

    /// Called when transcription finishes (success or failure) and the
    /// post-paste flow is done. If a recording was queued, it is drained.
    /// Returns the action the caller should perform.
    @discardableResult
    public func onTranscriptionCompleted() -> RecordingAction {
        isBusy = false
        if hasPendingRecording {
            hasPendingRecording = false
            return .startRecording
        }
        return .doNothing
    }

    // MARK: - Queue management

    /// Called when the user requests a new recording (hotkey press).
    /// If idle, returns `.startRecording` to begin immediately.
    /// If busy (transcribing), queues the recording and returns `.doNothing`.
    /// If already recording or pending, returns `.doNothing`.
    @discardableResult
    public func requestRecording() -> RecordingAction {
        guard !isRecording else { return .doNothing }
        guard !hasPendingRecording else { return .doNothing }

        if isBusy {
            hasPendingRecording = true
            return .doNothing
        }

        return .startRecording
    }

    /// Cancel any pending queued recording (e.g. on error without retry).
    public func cancelPending() {
        hasPendingRecording = false
    }

    /// Called when a manual retry transcription begins.
    /// Sets busy state so new requests are queued during the retry.
    public func onRetryStarted() {
        isBusy = true
    }

    /// Called when the user dismisses a failed-retry overlay by pressing
    /// the hotkey. Clears busy without draining the queue (the error path
    /// already called `cancelPending`, so no stale pending recording exists).
    public func clearBusyAfterRetry() {
        isBusy = false
    }

    /// Reset all state (e.g. on app-level reset).
    public func reset() {
        isRecording = false
        isBusy = false
        hasPendingRecording = false
    }
}
