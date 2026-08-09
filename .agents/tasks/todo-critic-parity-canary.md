# Critic gate: adversarial objective audit

Role: Critic
Objective: adversarially test whether the current diff actually meets the user's Rust-vs-Swift parity goal and whether any claim is only a proxy (unit test, helper test, or stale binary).
Owned paths: current git diff, task evidence, Computer Use canary contract, parity sources; read-only.
Acceptance proof: verdict CONTINUE or STOP with concrete missing proof and smallest required next action.
Excluded: do not edit, stage, commit, launch apps, or change permissions/TCC.
Stop conditions: finish after independent audit.

## Critic audit 2026-08-09

Verdict: STOP

### Decisive evidence

- `Tests/computer_use_macos.mjs:114-115,166-181` starts one executable but does not record its PID, bundle identifier, executable hash, or helper hash. The task contract requires proof against a fresh exact binary; an `appPath` argument alone cannot exclude a stale or wrong bundle.
- The runner's default helper is hard-coded to `/Applications/VoicePaste.app/Contents/MacOS/modifier_monitor` at `Tests/computer_use_macos.mjs:108`, while the app helper derived from `appPath` at line 115 is unused. The canary can therefore pass with a different, previously authorized helper identity than the app under test.
- `Tests/computer_use_macos.mjs:200-208` captures/emits a screenshot but `:226-230` deletes the temporary directory and no evidence JSON is written. The screenshot is not asserted to contain the compact overlay, and there is no AX assertion for recording/等待/result/error state. The final assertion at `:215-217` only proves the expected text eventually appeared in TextEdit.
- The current task evidence claims a latest Rust `PASS`, but no saved screenshot/AX/text evidence or actual invocation receipt is present in this Critic task. The selected `stop_when` also requires a comparative Swift baseline; the supplied evidence contains no Swift Computer Use run or equivalent saved baseline.
- Static checks are green: `node --check Tests/computer_use_macos.mjs` and `git diff --check` completed successfully. These are syntax/diff proxies only and do not close the UI proof gap.

### Excluded hypotheses

- This is not evidence that the Rust paste path is absent: `lib.rs:392-413` calls production paste and logs success/failure, and `pasteboard_typer.rs:221-255` invokes the helper with the captured PID.
- This is not evidence that Native STT readiness is still unimplemented: `native_stt.rs:255-261` now checks helper discovery and Speech authorization. The remaining block is parity/canary proof, not a claim that this implementation is correct in every runtime state.

### QUESTIONS_FOR_L

- Where is the immutable receipt for the claimed Rust PASS, including exact app PID/path/hash, helper path/hash/identity, screenshot, overlay/AX states, and final TextEdit value?
- Where is the Swift baseline run required by `work-rust-swift-parity-computer-use.md`'s `stop_when`? If it was intentionally excluded, who approved changing that acceptance condition?

### Two routes to proceed

1. Make the runner self-contained and rerun it: use the helper adjacent to the supplied `appPath`, record app/helper PID/path/bundle/hash, assert recording and result overlay states, and persist a redacted evidence bundle including screenshot, AX snapshots, and final text. Then run the same contract against Swift or attach an approved equivalent baseline.
2. If the existing production helper must remain the authorized helper, explicitly mark that as a controlled external dependency and provide a separate identity receipt proving it is the intended helper; add an independent overlay-state assertion and durable evidence export, then obtain a user-approved waiver for the missing Swift baseline.

### Minimum proof required

Do not claim completion or continue to the final gate until the above QUESTIONS_FOR_L are answered and a fresh Rust plus Swift comparative canary receipt exists. The receipt must demonstrate `Fn down → compact recording overlay → Fn up → transcription/result state → PID-targeted paste`, with persisted screenshot/AX/text evidence and exact binary/helper identities.
