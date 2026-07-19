/// Pure state machine for recording queue lifecycle.
/// No I/O, no timers — just state transitions and action results.

/// What the caller should do after a state transition.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecordingAction {
    /// Start recording immediately.
    StartRecording,
    /// Show "waiting" indicator (transcription in progress).
    ShowWaiting,
    /// Nothing to do.
    DoNothing,
}

/// Pure state machine that manages the recording → transcription → queue lifecycle.
///
/// States:
/// - Idle: not recording, not busy
/// - Recording: actively recording audio
/// - Busy: transcribing (or retrying)
/// - Recording + Pending: recording while transcription is in progress
pub struct RecordingQueueCoordinator {
    pub is_recording: bool,
    pub is_busy: bool,
    has_pending_recording: bool,
}

impl RecordingQueueCoordinator {
    pub fn new() -> Self {
        Self {
            is_recording: false,
            is_busy: false,
            has_pending_recording: false,
        }
    }

    /// User pressed the hotkey to start recording.
    pub fn request_recording(&mut self) -> RecordingAction {
        if self.is_recording {
            return RecordingAction::DoNothing;
        }
        if self.is_busy {
            // Queue it — will start when transcription finishes.
            self.has_pending_recording = true;
            return RecordingAction::DoNothing;
        }
        // Idle — start immediately.
        self.is_recording = true;
        RecordingAction::StartRecording
    }

    /// Recording hardware has actually started.
    pub fn on_recording_started(&mut self) {
        self.is_recording = true;
    }

    /// User released the hotkey — recording stopped, transcription begins.
    pub fn on_recording_stopped(&mut self) {
        self.is_recording = false;
        self.is_busy = true;
    }

    /// Transcription completed (success or final failure).
    /// Returns action for pending recording if any.
    pub fn on_transcription_completed(&mut self) -> RecordingAction {
        if self.has_pending_recording {
            self.has_pending_recording = false;
            self.is_recording = true;
            self.is_busy = false;
            return RecordingAction::StartRecording;
        }
        self.is_busy = false;
        RecordingAction::DoNothing
    }

    /// Manual retry started.
    pub fn on_retry_started(&mut self) {
        self.is_busy = true;
    }

    /// Clear busy state after retry completes (for drain purposes).
    pub fn clear_busy_after_retry(&mut self) {
        self.is_busy = false;
    }

    /// Whether there's a queued recording waiting.
    pub fn has_pending(&self) -> bool {
        self.has_pending_recording
    }

    /// Cancel any pending recording request.
    pub fn cancel_pending(&mut self) {
        self.has_pending_recording = false;
    }

    /// Full reset to idle state.
    pub fn reset(&mut self) {
        self.is_recording = false;
        self.is_busy = false;
        self.has_pending_recording = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_initial_state_is_idle() {
        let q = RecordingQueueCoordinator::new();
        assert!(!q.is_recording);
        assert!(!q.is_busy);
        assert!(!q.has_pending());
    }

    #[test]
    fn test_request_recording_when_idle_starts_immediately() {
        let mut q = RecordingQueueCoordinator::new();
        let action = q.request_recording();
        assert_eq!(action, RecordingAction::StartRecording);
        assert!(q.is_recording);
    }

    #[test]
    fn test_request_recording_when_already_recording_does_nothing() {
        let mut q = RecordingQueueCoordinator::new();
        q.request_recording();
        let action = q.request_recording();
        assert_eq!(action, RecordingAction::DoNothing);
    }

    #[test]
    fn test_request_recording_when_busy_queues() {
        let mut q = RecordingQueueCoordinator::new();
        q.request_recording();
        q.on_recording_stopped(); // now busy
        let action = q.request_recording();
        assert_eq!(action, RecordingAction::DoNothing);
        assert!(q.has_pending());
    }

    #[test]
    fn test_on_recording_stopped_sets_busy() {
        let mut q = RecordingQueueCoordinator::new();
        q.request_recording();
        q.on_recording_stopped();
        assert!(!q.is_recording);
        assert!(q.is_busy);
    }

    #[test]
    fn test_on_transcription_completed_clears_busy() {
        let mut q = RecordingQueueCoordinator::new();
        q.request_recording();
        q.on_recording_stopped();
        let action = q.on_transcription_completed();
        assert_eq!(action, RecordingAction::DoNothing);
        assert!(!q.is_busy);
    }

    #[test]
    fn test_on_transcription_completed_with_pending_starts_recording() {
        let mut q = RecordingQueueCoordinator::new();
        q.request_recording();
        q.on_recording_stopped();
        q.request_recording(); // queued
        let action = q.on_transcription_completed();
        assert_eq!(action, RecordingAction::StartRecording);
        assert!(q.is_recording);
        assert!(!q.is_busy);
        assert!(!q.has_pending());
    }

    #[test]
    fn test_full_queue_cycle() {
        let mut q = RecordingQueueCoordinator::new();
        // First recording
        assert_eq!(q.request_recording(), RecordingAction::StartRecording);
        q.on_recording_stopped();
        // Queue second while busy
        assert_eq!(q.request_recording(), RecordingAction::DoNothing);
        assert!(q.has_pending());
        // First transcription done → second starts
        assert_eq!(q.on_transcription_completed(), RecordingAction::StartRecording);
        assert!(q.is_recording);
        q.on_recording_stopped();
        // Second transcription done → idle
        assert_eq!(q.on_transcription_completed(), RecordingAction::DoNothing);
        assert!(!q.is_busy);
        assert!(!q.has_pending());
    }

    #[test]
    fn test_retry_lifecycle() {
        let mut q = RecordingQueueCoordinator::new();
        q.request_recording();
        q.on_recording_stopped();
        // Transcription failed, user retries
        q.on_retry_started();
        assert!(q.is_busy);
        // Queue a recording while retrying
        q.request_recording();
        assert!(q.has_pending());
        // Retry done
        assert_eq!(q.on_transcription_completed(), RecordingAction::StartRecording);
    }

    #[test]
    fn test_cancel_pending() {
        let mut q = RecordingQueueCoordinator::new();
        q.request_recording();
        q.on_recording_stopped();
        q.request_recording(); // queued
        assert!(q.has_pending());
        q.cancel_pending();
        assert!(!q.has_pending());
        let action = q.on_transcription_completed();
        assert_eq!(action, RecordingAction::DoNothing);
    }

    #[test]
    fn test_reset_clears_all() {
        let mut q = RecordingQueueCoordinator::new();
        q.request_recording();
        q.on_recording_stopped();
        q.request_recording();
        q.reset();
        assert!(!q.is_recording);
        assert!(!q.is_busy);
        assert!(!q.has_pending());
    }

    #[test]
    fn test_request_recording_when_both_recording_and_busy_does_nothing() {
        let mut q = RecordingQueueCoordinator::new();
        q.is_recording = true;
        q.is_busy = true;
        let action = q.request_recording();
        assert_eq!(action, RecordingAction::DoNothing);
    }

    #[test]
    fn test_clear_busy_after_retry() {
        let mut q = RecordingQueueCoordinator::new();
        q.on_retry_started();
        assert!(q.is_busy);
        q.clear_busy_after_retry();
        assert!(!q.is_busy);
    }
}
