# Reviewer gate: Rust/Swift parity and Computer Use diff

Role: Reviewer
Objective: independently review the current implementation diff for correctness, scope, regressions, and evidence quality against the selected Maximally Ideal plan.
Owned paths: current git diff, Tests/computer_use_macos.mjs, VoicePasteTauri/src-tauri/, README files; read-only.
Acceptance proof: report actionable findings with severity, exact paths/lines, and whether the green Computer Use evidence is sufficient.
Excluded: do not edit, stage, commit, launch apps, or change permissions/TCC.
Stop conditions: finish after review; do not redesign unrelated work.

## Reviewer evidence 2026-08-09

- Scope reviewed: current diff, `Tests/computer_use_macos.mjs`, Rust Tauri sources, Swift NativeSTT bridge, settings changes, and README documentation. No files outside the assigned scope were modified by this review.
- Verification: `node --check Tests/computer_use_macos.mjs` passed; Rust `cargo test --manifest-path VoicePasteTauri/src-tauri/Cargo.toml` passed 76/76; `swift test --package-path VoicePasteTauri/src-tauri` passed 13/13; `git diff --check` passed.
- [P1] Acceptance evidence is insufficient for the selected `stop_when`: `Tests/computer_use_macos.mjs:196-208` captures and emits a screenshot, but never asserts that the recording overlay appeared, was compact, or transitioned to a compact result/error state. The only functional assertion is final TextEdit text at `:215-217`. Smallest fix: add observable AX/screenshot assertions for recording and final overlay states, and retain structured state evidence in the returned bundle.
- [P1] The canary does not implement the selected plan's identity proof. `Tests/computer_use_macos.mjs:145-176` records an app path and target PID, but not the spawned Rust PID, bundle identity, executable/helper hashes, or helper code-signing/TCC identity; `const helper` at `:112` is unused. A green TextEdit result therefore cannot prove that the intended fresh app/helper pair was exercised. Smallest fix: record and assert `appProcess.pid`, bundle ID, and stable executable/helper identity (for example `codesign -dv`/hash output) before the hotkey.
- [P2] `run({ targetApp })` is not internally consistent: `Tests/computer_use_macos.mjs:150-153` always obtains a PID via `pgrep -x TextEdit`, while later AX operations use the caller-provided `targetApp`. A non-default target can inspect one app while production paste is posted to TextEdit. Smallest fix: derive the PID from the selected target app or reject non-TextEdit targets explicitly.
- The source changes had no direct regression found within scope. The paste path is exercised by the reported green canary evidence, and the Rust/Swift unit suites are green; however, the Computer Use evidence is not sufficient to approve the selected maximally-ideal gate.

## Verdict

CHANGES_REQUIRED

Unverified assumptions: the implementation evidence reports a live green canary, but no independently inspectable evidence bundle or overlay assertions were present in the repository for this review.
