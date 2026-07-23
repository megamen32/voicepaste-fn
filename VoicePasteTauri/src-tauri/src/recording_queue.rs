/// Lightweight tracker for the recording/transcription lifecycle.
///
/// Recording and transcription are deliberately independent: stopping one
/// recording increments `in_flight`, but a new recording is allowed to start
/// immediately while that work runs on a background thread.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecordingAction {
    StartRecording,
    ShowWaiting,
    DoNothing,
}

pub struct RecordingQueueCoordinator {
    pub is_recording: bool,
    pub is_busy: bool,
    pub in_flight: usize,
}

impl RecordingQueueCoordinator {
    pub fn new() -> Self {
        Self {
            is_recording: false,
            is_busy: false,
            in_flight: 0,
        }
    }

    /// A new recording is never queued behind an older transcription.
    pub fn request_recording(&mut self) -> RecordingAction {
        if self.is_recording {
            return RecordingAction::DoNothing;
        }
        self.is_recording = true;
        RecordingAction::StartRecording
    }

    pub fn on_recording_started(&mut self) {
        self.is_recording = true;
    }

    pub fn on_recording_stopped(&mut self) {
        self.is_recording = false;
        self.in_flight += 1;
        self.is_busy = true;
    }

    pub fn on_transcription_completed(&mut self) -> RecordingAction {
        self.in_flight = self.in_flight.saturating_sub(1);
        self.is_busy = self.in_flight > 0;
        RecordingAction::DoNothing
    }

    pub fn on_retry_started(&mut self) {
        self.in_flight += 1;
        self.is_busy = true;
    }

    pub fn clear_busy_after_retry(&mut self) {
        self.in_flight = self.in_flight.saturating_sub(1);
        self.is_busy = self.in_flight > 0;
    }

    /// Kept as a compatibility no-op for callers from the old queued design.
    pub fn has_pending(&self) -> bool {
        false
    }

    pub fn cancel_pending(&mut self) {}

    pub fn reset(&mut self) {
        self.is_recording = false;
        self.is_busy = false;
        self.in_flight = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn initial_state_is_idle() {
        let q = RecordingQueueCoordinator::new();
        assert!(!q.is_recording);
        assert!(!q.is_busy);
        assert_eq!(q.in_flight, 0);
    }

    #[test]
    fn recording_starts_when_idle() {
        let mut q = RecordingQueueCoordinator::new();
        assert_eq!(q.request_recording(), RecordingAction::StartRecording);
        assert!(q.is_recording);
    }

    #[test]
    fn repeated_start_while_recording_is_ignored() {
        let mut q = RecordingQueueCoordinator::new();
        q.request_recording();
        assert_eq!(q.request_recording(), RecordingAction::DoNothing);
    }

    #[test]
    fn new_recording_starts_while_previous_transcription_is_busy() {
        let mut q = RecordingQueueCoordinator::new();
        q.request_recording();
        q.on_recording_stopped();
        assert_eq!(q.in_flight, 1);
        assert_eq!(q.request_recording(), RecordingAction::StartRecording);
        assert!(q.is_recording);
    }

    #[test]
    fn concurrent_transcriptions_drain_independently() {
        let mut q = RecordingQueueCoordinator::new();
        q.on_recording_stopped();
        q.on_recording_started();
        q.on_recording_stopped();
        assert_eq!(q.in_flight, 2);
        q.on_transcription_completed();
        assert!(q.is_busy);
        assert_eq!(q.in_flight, 1);
        q.on_transcription_completed();
        assert!(!q.is_busy);
        assert_eq!(q.in_flight, 0);
    }

    #[test]
    fn retry_is_counted_as_background_work() {
        let mut q = RecordingQueueCoordinator::new();
        q.on_retry_started();
        assert!(q.is_busy);
        q.clear_busy_after_retry();
        assert!(!q.is_busy);
        assert_eq!(q.in_flight, 0);
    }

    #[test]
    fn reset_clears_all_state() {
        let mut q = RecordingQueueCoordinator::new();
        q.on_recording_stopped();
        q.reset();
        assert!(!q.is_recording);
        assert!(!q.is_busy);
        assert_eq!(q.in_flight, 0);
    }
}
