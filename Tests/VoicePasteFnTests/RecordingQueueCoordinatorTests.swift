import XCTest
@testable import VoicePasteLib

final class RecordingQueueCoordinatorTests: XCTestCase {

    var sut: RecordingQueueCoordinator!

    override func setUp() {
        super.setUp()
        sut = RecordingQueueCoordinator()
    }

    override func tearDown() {
        sut = nil
        super.tearDown()
    }

    // MARK: - Basic recording lifecycle

    func testInitialState_isIdle() {
        XCTAssertFalse(sut.isRecording)
        XCTAssertFalse(sut.isBusy)
        XCTAssertFalse(sut.hasPendingRecording)
    }

    func testRequestRecording_whenIdle_startsImmediately() {
        let action = sut.requestRecording()
        XCTAssertEqual(action, .startRecording)
    }

    func testOnRecordingStarted_setsIsRecording() {
        sut.onRecordingStarted()
        XCTAssertTrue(sut.isRecording)
    }

    func testOnRecordingStopped_clearsRecordingAndSetsBusy() {
        sut.onRecordingStarted()
        let action = sut.onRecordingStopped()
        XCTAssertEqual(action, .showWaiting)
        XCTAssertFalse(sut.isRecording)
        XCTAssertTrue(sut.isBusy)
    }

    func testOnTranscriptionCompleted_clearsBusy() {
        sut.onRecordingStarted()
        sut.onRecordingStopped()
        let action = sut.onTranscriptionCompleted()
        XCTAssertEqual(action, .doNothing)
        XCTAssertFalse(sut.isBusy)
    }

    // MARK: - Queue: request while busy

    func testRequestRecording_whenBusy_queuesRecording() {
        sut.onRecordingStarted()
        sut.onRecordingStopped()
        XCTAssertTrue(sut.isBusy)

        let action = sut.requestRecording()
        XCTAssertEqual(action, .doNothing, "Should not start recording while busy")
        XCTAssertTrue(sut.hasPendingRecording, "Should queue the recording")
    }

    func testOnTranscriptionCompleted_withPending_drainsQueue() {
        sut.onRecordingStarted()
        sut.onRecordingStopped()
        sut.requestRecording()
        XCTAssertTrue(sut.hasPendingRecording)

        let action = sut.onTranscriptionCompleted()
        XCTAssertEqual(action, .startRecording, "Should drain the queue and start recording")
        XCTAssertFalse(sut.hasPendingRecording, "Pending flag should be cleared")
        XCTAssertFalse(sut.isBusy, "Busy should be cleared")
    }

    func testOnTranscriptionCompleted_withoutPending_doesNothing() {
        sut.onRecordingStarted()
        sut.onRecordingStopped()

        let action = sut.onTranscriptionCompleted()
        XCTAssertEqual(action, .doNothing)
    }

    // MARK: - Edge cases

    func testRequestRecording_whenAlreadyRecording_doesNothing() {
        sut.onRecordingStarted()
        let action = sut.requestRecording()
        XCTAssertEqual(action, .doNothing)
        XCTAssertFalse(sut.hasPendingRecording)
    }

    func testRequestRecording_whenAlreadyPending_doesNotDoubleQueue() {
        sut.onRecordingStarted()
        sut.onRecordingStopped()
        sut.requestRecording()
        let action = sut.requestRecording()
        XCTAssertEqual(action, .doNothing)
        XCTAssertTrue(sut.hasPendingRecording)
    }

    func testCancelPending_clearsPendingFlag() {
        sut.onRecordingStarted()
        sut.onRecordingStopped()
        sut.requestRecording()
        XCTAssertTrue(sut.hasPendingRecording)

        sut.cancelPending()
        XCTAssertFalse(sut.hasPendingRecording)

        let action = sut.onTranscriptionCompleted()
        XCTAssertEqual(action, .doNothing)
    }

    func testReset_clearsAllState() {
        sut.onRecordingStarted()
        sut.onRecordingStopped()
        sut.requestRecording()

        sut.reset()
        XCTAssertFalse(sut.isRecording)
        XCTAssertFalse(sut.isBusy)
        XCTAssertFalse(sut.hasPendingRecording)
    }

    // MARK: - Full queue cycle

    func testFullQueueCycle() {
        XCTAssertEqual(sut.requestRecording(), .startRecording)
        sut.onRecordingStarted()

        XCTAssertEqual(sut.onRecordingStopped(), .showWaiting)
        XCTAssertTrue(sut.isBusy)

        XCTAssertEqual(sut.requestRecording(), .doNothing)
        XCTAssertTrue(sut.hasPendingRecording)

        XCTAssertEqual(sut.onTranscriptionCompleted(), .startRecording)
        XCTAssertFalse(sut.hasPendingRecording)
        XCTAssertFalse(sut.isBusy)

        sut.onRecordingStarted()
        XCTAssertTrue(sut.isRecording)

        sut.onRecordingStopped()
        XCTAssertEqual(sut.onTranscriptionCompleted(), .doNothing)
        XCTAssertFalse(sut.isBusy)
        XCTAssertFalse(sut.isRecording)
    }

    func testQueueWhileRecording_thenBusy() {
        sut.onRecordingStarted()
        XCTAssertEqual(sut.requestRecording(), .doNothing, "Cannot queue while recording")
        XCTAssertFalse(sut.hasPendingRecording)

        sut.onRecordingStopped()
        XCTAssertTrue(sut.isBusy)

        XCTAssertEqual(sut.requestRecording(), .doNothing)
        XCTAssertTrue(sut.hasPendingRecording)

        XCTAssertEqual(sut.onTranscriptionCompleted(), .startRecording)
    }

    // MARK: - Consecutive queue cycles

    func testConsecutiveQueueCycles() {
        // Cycle 1: record → transcribe → queue → drain
        XCTAssertEqual(sut.requestRecording(), .startRecording)
        sut.onRecordingStarted()
        sut.onRecordingStopped()
        sut.requestRecording()
        XCTAssertEqual(sut.onTranscriptionCompleted(), .startRecording)

        // Cycle 2: second recording starts → transcribe → queue → drain
        sut.onRecordingStarted()
        sut.onRecordingStopped()
        sut.requestRecording()
        XCTAssertEqual(sut.onTranscriptionCompleted(), .startRecording)

        // Cycle 3: clean finish with no pending
        sut.onRecordingStarted()
        sut.onRecordingStopped()
        XCTAssertEqual(sut.onTranscriptionCompleted(), .doNothing)
        XCTAssertFalse(sut.isBusy)
        XCTAssertFalse(sut.isRecording)
        XCTAssertFalse(sut.hasPendingRecording)
    }

    // MARK: - State safety

    func testOnTranscriptionCompleted_whenIdle_returnsDoNothing() {
        // Calling completion from a fully idle state is safe
        let action = sut.onTranscriptionCompleted()
        XCTAssertEqual(action, .doNothing)
        XCTAssertFalse(sut.isBusy)
    }

    func testOnTranscriptionCompleted_calledTwice_returnsDoNothingSecondTime() {
        sut.onRecordingStarted()
        sut.onRecordingStopped()
        sut.requestRecording()
        XCTAssertEqual(sut.onTranscriptionCompleted(), .startRecording)
        // Second call: pending already drained, should do nothing
        XCTAssertEqual(sut.onTranscriptionCompleted(), .doNothing)
    }

    func testCancelPending_whenNothingPending_isNoOp() {
        sut.cancelPending()
        XCTAssertFalse(sut.hasPendingRecording)
    }

    func testRequestRecording_afterCancelPending_whileBusy_queuesAgain() {
        sut.onRecordingStarted()
        sut.onRecordingStopped()
        sut.requestRecording()
        sut.cancelPending()
        XCTAssertFalse(sut.hasPendingRecording)

        // Should be able to queue again after cancellation
        XCTAssertEqual(sut.requestRecording(), .doNothing)
        XCTAssertTrue(sut.hasPendingRecording)
    }

    func testRequestRecording_afterTranscriptionCompleted_startsImmediately() {
        sut.onRecordingStarted()
        sut.onRecordingStopped()
        sut.onTranscriptionCompleted()
        XCTAssertFalse(sut.isBusy)

        // Now idle — should start immediately
        XCTAssertEqual(sut.requestRecording(), .startRecording)
    }

    func testReset_whileRecording_clearsEverything() {
        sut.onRecordingStarted()
        XCTAssertTrue(sut.isRecording)
        sut.reset()
        XCTAssertFalse(sut.isRecording)
        XCTAssertFalse(sut.isBusy)
        XCTAssertFalse(sut.hasPendingRecording)
    }

    func testRequestRecording_whenBothRecordingAndBusy_doesNothing() {
        sut.onRecordingStarted()     // isRecording = true
        sut.onRecordingStopped()     // isRecording = false, isBusy = true
        sut.isRecording = true       // force both true
        let action = sut.requestRecording()
        XCTAssertEqual(action, .doNothing)
        XCTAssertFalse(sut.hasPendingRecording)
    }

    // MARK: - Rapid-fire queueing

    func testMultipleRequestRecording_whenBusy_onlyQueuesOnce() {
        sut.onRecordingStarted()
        sut.onRecordingStopped()
        XCTAssertTrue(sut.isBusy)

        // Spam requests
        for _ in 0..<10 {
            _ = sut.requestRecording()
        }
        XCTAssertTrue(sut.hasPendingRecording)

        // Single drain
        XCTAssertEqual(sut.onTranscriptionCompleted(), .startRecording)
        XCTAssertFalse(sut.hasPendingRecording)
    }

    func testDrainThenRequest_startsImmediately() {
        sut.onRecordingStarted()
        sut.onRecordingStopped()
        sut.requestRecording()
        XCTAssertEqual(sut.onTranscriptionCompleted(), .startRecording)
        XCTAssertFalse(sut.isBusy)

        // After drain, requesting again should start immediately
        XCTAssertEqual(sut.requestRecording(), .startRecording)
    }

    // MARK: - Retry lifecycle

    func testOnRetryStarted_setsBusy() {
        sut.onRetryStarted()
        XCTAssertTrue(sut.isBusy)
    }

    func testOnRetryStarted_allowsQueueing() {
        sut.onRetryStarted()
        XCTAssertEqual(sut.requestRecording(), .doNothing)
        XCTAssertTrue(sut.hasPendingRecording)
    }

    func testOnRetryStarted_thenTranscriptionCompleted_drainsQueue() {
        sut.onRetryStarted()
        sut.requestRecording()
        XCTAssertTrue(sut.hasPendingRecording)

        XCTAssertEqual(sut.onTranscriptionCompleted(), .startRecording)
        XCTAssertFalse(sut.hasPendingRecording)
        XCTAssertFalse(sut.isBusy)
    }

    func testClearBusyAfterRetry_clearsBusyWithoutDraining() {
        sut.onRetryStarted()
        XCTAssertTrue(sut.isBusy)

        sut.clearBusyAfterRetry()
        XCTAssertFalse(sut.isBusy)
        XCTAssertFalse(sut.hasPendingRecording)
    }

    func testClearBusyAfterRetry_whenIdle_isNoOp() {
        sut.clearBusyAfterRetry()
        XCTAssertFalse(sut.isBusy)
    }

    func testClearBusyAfterRetry_doesNotAffectPendingRecording() {
        sut.onRetryStarted()
        sut.requestRecording()
        XCTAssertTrue(sut.hasPendingRecording)

        // clearBusyAfterRetry only clears busy, not pending
        sut.clearBusyAfterRetry()
        XCTAssertFalse(sut.isBusy)
        XCTAssertTrue(sut.hasPendingRecording)

        // The pending recording should still drain on completion
        XCTAssertEqual(sut.onTranscriptionCompleted(), .startRecording)
    }

    // MARK: - Immediate drain contract

    /// When transcription completes with a pending recording, the coordinator
    /// must be in a state where recording can start IMMEDIATELY — no delay,
    /// no debounce. isBusy=false, isRecording=false, hasPendingRecording=false.
    func testOnTranscriptionCompleted_withPending_isReadyForImmediateRecording() {
        sut.onRecordingStarted()
        sut.onRecordingStopped()
        sut.requestRecording()

        let action = sut.onTranscriptionCompleted()
        XCTAssertEqual(action, .startRecording)
        XCTAssertFalse(sut.isBusy, "Must not be busy — recording should start immediately")
        XCTAssertFalse(sut.isRecording, "Must not already be recording")
        XCTAssertFalse(sut.hasPendingRecording, "Pending must be cleared before recording starts")
    }

    /// After draining and starting the new recording, the full lifecycle
    /// must work normally (stop → transcribe → complete).
    func testDrainRecording_fullLifecycleWorks() {
        // First cycle: record → transcribe → queue
        sut.onRecordingStarted()
        sut.onRecordingStopped()
        sut.requestRecording()
        XCTAssertEqual(sut.onTranscriptionCompleted(), .startRecording)

        // Drain: start new recording immediately
        sut.onRecordingStarted()
        XCTAssertTrue(sut.isRecording)
        XCTAssertFalse(sut.isBusy)

        // Second cycle: stop → transcribe → complete (no pending)
        sut.onRecordingStopped()
        XCTAssertTrue(sut.isBusy)
        XCTAssertFalse(sut.isRecording)
        XCTAssertEqual(sut.onTranscriptionCompleted(), .doNothing)
    }

    /// If the user cancels pending before drain, no recording should start.
    func testCancelPending_beforeDrain_preventsRecording() {
        sut.onRecordingStarted()
        sut.onRecordingStopped()
        sut.requestRecording()
        sut.cancelPending()

        XCTAssertEqual(sut.onTranscriptionCompleted(), .doNothing)
        XCTAssertFalse(sut.isBusy)
        XCTAssertFalse(sut.isRecording)
    }
}
