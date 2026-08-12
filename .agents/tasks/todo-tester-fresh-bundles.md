# Tester gate: exact fresh bundles only-new

Role: Tester
Objective: independently run the parity Computer Use canary using exactly the freshly built workspace bundles below, then inspect durable receipts.
Original goal: Rust VoicePaste must reach Swift parity and have a real UI canary.
Allowed paths: Tests/computer_use_macos.mjs, exact fresh bundles, `.agents/evidence/computer-use/`, real TextEdit target; read-only to source.
Excluded: do not use `/Applications/VoicePaste.app`, archived artifacts, debug bundles, or any stale binary; do not edit, commit, change TCC, or change persistent settings.
Exact Rust app: `/Users/user/Documents/Apps/voicepaste-fn-minimal3/VoicePasteTauri/src-tauri/target/release/bundle/macos/VoicePaste.app`
Exact Swift app: `/Users/user/Documents/Apps/voicepaste-fn-minimal3/build/VoicePasteFn.app`
Acceptance proof: both implementations pass hotkey → compact overlay → waiting/result lifecycle → exact AX paste, and receipts show the exact paths, PIDs, hashes, codesign and bundle IDs.
Stop conditions: report exact blocker; do not repair during Tester pass.

## Tester evidence (2026-08-09)

Surface/tool: real macOS applications via `node_repl` + `@oai/sky`, with real TextEdit target and `Tests/computer_use_macos.mjs`. Mode: `only-new`.

Exact journey exercised for each implementation: launch the specified fresh bundle; clear TextEdit editable AX element; inject Fn down/up; observe compact recording overlay; observe waiting/result lifecycle; verify the deterministic fixture transcription is pasted into the real TextEdit field; inspect the generated redacted receipt.

Final exact-bundle results:

- `PASS` Rust. Receipt: `.agents/evidence/computer-use/1786274074028-rust/evidence.json`. App path is the specified workspace release bundle; helper path is also inside that same bundle. App PID `54492`, TextEdit PID `24857`; executable SHA-256 `4272089339ea718521b472d767a58e2c04f7a41e86f9ea9a7b9044592de1a56a`; helper SHA-256 `c3561da944ca397d31ac6159b0909127f29c16fae08cedbd0b0e68f3f405cb16`; executable bundle ID `com.bezrabotnyi.voicepaste`; overlay log proves `recording 72x56`, `waiting 64x44`, `preview 360x100`; final AX value equals the generated expected fixture text.
- `PASS` Swift. Receipt: `.agents/evidence/computer-use/1786274084223-swift/evidence.json`. App path is the specified workspace `build/VoicePasteFn.app`. App PID `54796`, TextEdit PID `24857`; executable SHA-256 `a3ce315b2ed0d50fb373eb1e67612530efcf4bc79c9c9fb2a7e24363ea400c80`; executable bundle ID `com.bezrabotnyi.voicepastefn`; AX overlay probe proves onscreen compact recording `58x38` and result `297x38`; final AX value equals the generated expected fixture text.

Observed issue during testing: an initial Rust invocation used the script default `/Applications/VoicePaste.app/Contents/MacOS/modifier_monitor`, which is excluded by this task. That run is not acceptance evidence. I reran Rust with the helper explicitly set to the exact fresh workspace bundle; the final receipt above is the accepted result. No source, TCC, persistent settings, or app bundle was modified by Tester.

Verdict: `PASS`
