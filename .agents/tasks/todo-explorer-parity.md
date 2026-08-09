# Explorer lane: Rust/Swift parity review

Role: Explorer
Objective: inspect committed Rust/Swift paths for paste insertion, Native STT readiness, cascade/model/language parity, and overlay differences; produce evidence-backed fix candidates.
Original goal: Rust VoicePaste must reach Swift parity and have a real UI canary.
Owned paths: Sources/, VoicePasteTauri/src-tauri/src/, VoicePasteTauri/src-tauri/Sources/; read-only.
Excluded: do not edit source, do not change permissions/TCC, do not launch or modify external apps, do not commit.
Acceptance proof: report exact file/line evidence, minimal proposed changes, and a focused red-canary contract for each behavior.
Stop conditions: stop after the independent parity map and do not implement.
Budget: 20 / 40 / 75 active minutes; medium relative cost.

## Explorer evidence (2026-08-09)

Scope checked read-only: `Sources/`, `VoicePasteTauri/src-tauri/src/`, and
`VoicePasteTauri/src-tauri/Sources/`. No source files were modified, no
permissions/TCC were changed, and no external app was launched.

### Findings

1. **Paste insertion is not symmetric.** Rust captures the frontmost PID before
   recording (`VoicePasteTauri/src-tauri/src/lib.rs:265-267`), then calls
   `PasteboardTyper::paste_to_pid` after transcription (`lib.rs:392`, `lib.rs:498`).
   The macOS helper accepts `--pid` and posts Cmd+V directly to that process
   (`VoicePasteTauri/src-tauri/src/pasteboard_typer.rs:229-249`). Swift standalone
   calls `self.typer.paste(result)` with no target PID
   (`Sources/VoicePasteFn/VoicePasteApp.swift:357-360`, retry at 415-419); its
   `PasteboardTyper.paste` writes NSPasteboard and posts to the current HID target
   (`Sources/VoicePasteFn/main.swift:272-289`). This can paste into the overlay or
   another app after focus changes, and the Swift method has no error result.

   **Red canary contract:** focus a real text field in app A, start recording,
   focus app B while transcription is pending, then finish. The text must land in
   app A; a failed paste must produce an observable error state rather than being
   reported as success.

2. **Rust Native STT is wired end-to-end, but availability is optimistic.** The
   Swift helper uses `SFSpeechRecognizer`, checks authorization, locale,
   recognizer availability, file decoding, on-device capability, and a 30-second
   timeout (`VoicePasteTauri/src-tauri/Sources/NativeSTT/NativeSTTService.swift:55-180`).
   It emits a JSON result on stdout or structured error JSON on stderr
   (`NativeSTTExec/main.swift:1-53`, `79-101`), and Rust parses that process result
   (`VoicePasteTauri/src-tauri/src/native_stt.rs:59-105`). However,
   `NativeSttService::is_available()` returns `true` on every macOS host without
   checking authorization or helper existence (`native_stt.rs:223-232`). The
   tray therefore exposes Native even when the helper is absent or Speech is
   denied; the first runtime call fails and only then falls through the cascade.
   The Rust comment at `native_stt.rs:230-231` is stale: it still says the binding
   does not exist although the Swift helper is present.

   **Red canary contract:** on macOS with the helper absent, and separately with
   Speech authorization denied, the settings/tray availability result must not
   claim Native is usable; selecting a cascade containing Native must skip it
   without spawning a doomed transcription process. With an authorized helper and
   a valid WAV, the same path must return non-empty recognized text or a structured
   actionable failure within 30 seconds.

3. **Helper lookup has a documented PATH mismatch.** Rust documentation says the
   bare `native_stt` fallback is searched on PATH (`native_stt.rs:116-120`), but
   implementation only accepts `PathBuf::from("native_stt")` when
   `bare.exists()` is true (`native_stt.rs:140-146`). A binary available only via
   PATH is therefore rejected before `Command::new` can resolve it. Production
   bundling is configured through `externalBin` (`VoicePasteTauri/src-tauri/tauri.conf.json:74-77`).

   **Red canary contract:** place an executable named `native_stt` in a temporary
   PATH directory but not the working directory; helper discovery must select and
   invoke it, while a missing helper must return a clear unavailable result.

4. **Language parity differs.** Rust `Language` supports ru/en/zh/auto and passes
   `api_value()` to all cascade tiers (`VoicePasteTauri/src-tauri/src/models.rs:5-50`,
   `lib.rs:224-228`, `lib.rs:466-467`). Swift standalone exposes only ru/en/auto
   (`Sources/VoicePasteFn/SettingsModel.swift:166-186`); its Apple local fallback
   maps the caller's language code to `Locale(identifier:)`
   (`Sources/VoicePasteLib/LocalTranscriber.swift:18-35`). Thus Chinese is
   available in Rust UI/config but not in the Swift UI/fallback.

   **Red canary contract:** selecting zh must either be available consistently in
   both clients and reach the native recognizer as a valid locale, or be rejected
   consistently before recording; no silent fallback to auto/current locale.

5. **Cascade/overlay behavior is close but not identical.** Rust constructs tiers
   from persisted `engine_order`, skipping unavailable tiers and trying first
   success (`VoicePasteTauri/src-tauri/src/lib.rs:235-260`; cascade empty-result
   policy at `transcription_service.rs:11-101`). Default order is Remote → Native
   (`config.rs:53-66`). Swift standalone always constructs Remote with up to three
   retries and an optional single Apple `LocalTranscriber` fallback
   (`Sources/VoicePasteFn/VoicePasteApp.swift:308-315`); it has no Local/Native
   order control. Both show waiting/preview/retry states, but Rust records and
   surfaces paste errors (`lib.rs:389-409`), while Swift shows preview and calls a
   void paste method (`VoicePasteApp.swift:357-360`). Swift's `RecordingOverlay`
   is language-neutral and supports recording/waiting/error/retry states
   (`Sources/VoicePasteFn/RecordingOverlay.swift:16-49`, `195-281`), while Rust's
   OverlayManager is driven from the Tauri window path.

   **Red canary contract:** exercise remote failure → fallback success, all tiers
   failure, empty transcript, paste failure, and retry click. Each outcome must
   leave queue state consistent, show the matching overlay state, and never hide a
   newer recording indicator.

### Highest-value next probe

Run the focused black-box UI canary for target-PID paste and failure visibility
first. In parallel, add a helper-availability canary that distinguishes missing
binary, denied Speech authorization, unavailable locale, and successful authorized
recognition. These probes determine whether the next implementation slice should
prioritize the shared paste contract or Native availability/helper discovery.

## Result

Independent parity map complete. Findings and red-canary contracts are recorded
above; implementation is intentionally not performed in the Explorer lane.
