import XCTest
@testable import VoicePasteLib

final class MockTranscriptionService: TranscriptionService {
    var results: [Result<String, Error>] = []
    var callCount = 0
    func transcribe(fileURL: URL, languageCode: String?) throws -> String {
        defer { callCount += 1 }
        let idx = min(callCount, results.count - 1)
        guard idx >= 0, idx < results.count else { throw TranscriptionError.noResult }
        switch results[idx] {
        case .success(let text): return text
        case .failure(let error): throw error
        }
    }
}

enum TranscriptionError: Error, Equatable {
    case noResult, serverError, localError
}

final class CapturingLanguageService: TranscriptionService {
    var results: [Result<String, Error>] = []
    var callCount = 0
    var lastLanguageCode: String?
    func transcribe(fileURL: URL, languageCode: String?) throws -> String {
        lastLanguageCode = languageCode
        let idx = min(callCount, results.count - 1)
        callCount += 1
        guard idx >= 0, idx < results.count else { throw TranscriptionError.noResult }
        switch results[idx] {
        case .success(let text): return text
        case .failure(let error): throw error
        }
    }
}

final class RetryFallbackTests: XCTestCase {
    var primary: MockTranscriptionService!
    var fallback: MockTranscriptionService!
    override func setUp() { super.setUp(); primary = MockTranscriptionService(); fallback = MockTranscriptionService() }
    override func tearDown() { primary = nil; fallback = nil; super.tearDown() }

    func testPrimarySuccess_firstAttempt() {
        primary.results = [.success("hello")]
        let sut = RetryTranscriber(primary: primary, fallback: nil, maxAttempts: 3)
        XCTAssertEqual(try? sut.transcribe(fileURL: URL(fileURLWithPath: "/tmp/t.wav"), languageCode: "en"), "hello")
        XCTAssertEqual(primary.callCount, 1)
    }
    func testPrimarySuccess_secondAttempt() {
        primary.results = [.failure(TranscriptionError.serverError), .success("hello")]
        let sut = RetryTranscriber(primary: primary, fallback: nil, maxAttempts: 3)
        XCTAssertEqual(try? sut.transcribe(fileURL: URL(fileURLWithPath: "/tmp/t.wav"), languageCode: "en"), "hello")
        XCTAssertEqual(primary.callCount, 2)
    }
    func testPrimarySuccess_thirdAttempt() {
        primary.results = [.failure(TranscriptionError.serverError), .failure(TranscriptionError.serverError), .success("hello")]
        let sut = RetryTranscriber(primary: primary, fallback: nil, maxAttempts: 3)
        XCTAssertEqual(try? sut.transcribe(fileURL: URL(fileURLWithPath: "/tmp/t.wav"), languageCode: "en"), "hello")
        XCTAssertEqual(primary.callCount, 3)
    }
    func testAllAttemptsFail_noFallback_throws() {
        primary.results = [.failure(TranscriptionError.serverError), .failure(TranscriptionError.serverError), .failure(TranscriptionError.serverError)]
        let sut = RetryTranscriber(primary: primary, fallback: nil, maxAttempts: 3)
        XCTAssertThrowsError(try sut.transcribe(fileURL: URL(fileURLWithPath: "/tmp/t.wav"), languageCode: "en"))
        XCTAssertEqual(primary.callCount, 3)
    }
    func testAllAttemptsFail_fallbackEnabled_usesFallback() {
        primary.results = [.failure(TranscriptionError.serverError), .failure(TranscriptionError.serverError), .failure(TranscriptionError.serverError)]
        fallback.results = [.success("local result")]
        let sut = RetryTranscriber(primary: primary, fallback: fallback, maxAttempts: 3)
        XCTAssertEqual(try? sut.transcribe(fileURL: URL(fileURLWithPath: "/tmp/t.wav"), languageCode: "en"), "local result")
        XCTAssertEqual(primary.callCount, 3)
        XCTAssertEqual(fallback.callCount, 1)
    }
    func testAllAttemptsFail_fallbackAlsoFails_throws() {
        primary.results = [.failure(TranscriptionError.serverError), .failure(TranscriptionError.serverError), .failure(TranscriptionError.serverError)]
        fallback.results = [.failure(TranscriptionError.localError)]
        let sut = RetryTranscriber(primary: primary, fallback: fallback, maxAttempts: 3)
        XCTAssertThrowsError(try sut.transcribe(fileURL: URL(fileURLWithPath: "/tmp/t.wav"), languageCode: "en"))
        XCTAssertEqual(primary.callCount, 3)
        XCTAssertEqual(fallback.callCount, 1)
    }
    func testSuccessOnFirstAttempt_fallbackNotCalled() {
        primary.results = [.success("server result")]
        fallback.results = [.success("local result")]
        let sut = RetryTranscriber(primary: primary, fallback: fallback, maxAttempts: 3)
        XCTAssertEqual(try? sut.transcribe(fileURL: URL(fileURLWithPath: "/tmp/t.wav"), languageCode: "en"), "server result")
        XCTAssertEqual(primary.callCount, 1)
        XCTAssertEqual(fallback.callCount, 0)
    }
    func testSuccessOnSecondAttempt_fallbackNotCalled() {
        primary.results = [.failure(TranscriptionError.serverError), .success("server result")]
        fallback.results = [.success("local result")]
        let sut = RetryTranscriber(primary: primary, fallback: fallback, maxAttempts: 3)
        XCTAssertEqual(try? sut.transcribe(fileURL: URL(fileURLWithPath: "/tmp/t.wav"), languageCode: "en"), "server result")
        XCTAssertEqual(primary.callCount, 2)
        XCTAssertEqual(fallback.callCount, 0)
    }
    func testMaxAttemptsOne_failsThenFallback() {
        primary.results = [.failure(TranscriptionError.serverError)]
        fallback.results = [.success("local")]
        let sut = RetryTranscriber(primary: primary, fallback: fallback, maxAttempts: 1)
        XCTAssertEqual(try? sut.transcribe(fileURL: URL(fileURLWithPath: "/tmp/t.wav"), languageCode: "en"), "local")
        XCTAssertEqual(primary.callCount, 1)
        XCTAssertEqual(fallback.callCount, 1)
    }
    func testLanguageCodePassedToPrimary() {
        let captured = CapturingLanguageService()
        captured.results = [.success("ok")]
        let sut = RetryTranscriber(primary: captured, fallback: nil, maxAttempts: 3)
        _ = try? sut.transcribe(fileURL: URL(fileURLWithPath: "/tmp/t.wav"), languageCode: "ru")
        XCTAssertEqual(captured.lastLanguageCode, "ru")
    }
    func testLanguageCodePassedToFallback() {
        primary.results = [.failure(TranscriptionError.serverError), .failure(TranscriptionError.serverError), .failure(TranscriptionError.serverError)]
        let cf = CapturingLanguageService()
        cf.results = [.success("ok")]
        let sut = RetryTranscriber(primary: primary, fallback: cf, maxAttempts: 3)
        _ = try? sut.transcribe(fileURL: URL(fileURLWithPath: "/tmp/t.wav"), languageCode: "ru")
        XCTAssertEqual(cf.lastLanguageCode, "ru")
    }
    func testNilLanguageCode_autoDetect() {
        let captured = CapturingLanguageService()
        captured.results = [.success("ok")]
        let sut = RetryTranscriber(primary: captured, fallback: nil, maxAttempts: 3)
        _ = try? sut.transcribe(fileURL: URL(fileURLWithPath: "/tmp/t.wav"), languageCode: nil)
        XCTAssertNil(captured.lastLanguageCode)
    }
}
