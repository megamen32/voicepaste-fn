# Tester gate: only-new real product surface

Role: Tester
Objective: independently run the newly added Computer Use parity canary against the fresh Rust release bundle and fresh Swift baseline, then inspect the durable receipts and report only-new acceptance.
Original goal: Rust VoicePaste must reach Swift parity and have a real UI canary.
Allowed paths: Tests/computer_use_macos.mjs, fresh build bundles, `.agents/evidence/computer-use/`, real TextEdit target; read-only to source.
Excluded: do not edit source, do not commit, do not change TCC or persistent settings, do not use unit tests as substitute for UI evidence.
Acceptance proof: both implementations pass hotkey → compact overlay → waiting/result lifecycle → exact AX paste, with app PID, bundle identity, hashes and durable receipt.
Stop conditions: report exact blocker if either implementation cannot run; do not repair during Tester pass.

## Tester evidence — 2026-08-09

- Surface/tool: real macOS desktop UI via `node_repl` + `@oai/sky`, target `com.apple.TextEdit`, editable AX element 2. The allowed `Tests/computer_use_macos.mjs` canary was invoked against both fresh bundles. No source, TCC, or persistent settings were changed.
- Rust journey: spawn `/Applications/VoicePaste.app`, set TextEdit element 2 to the generated fixture target, inject Fn down. App and modifier helper started; app PID `52008`, target PID `24857`; modifier monitor logged `Modifier pressed: fn`. The compact recording overlay never appeared (`overlay_recording: []`), so the journey stopped before waiting/result and paste.
- Rust identity: executable SHA-256 `b03177f24984dbf1236f5a48485e3feb94813be86df3487495f7eb32651cc865`, bundle ID `com.bezrabotnyi.voicepaste`; helper SHA-256 `eff09e52449de15826e0ce88e9ee21f7a2a762a3e6e29b5d903d1e08135c5e57`. Durable receipt: `.agents/evidence/computer-use/1786273897425-rust/evidence.json`.
- Rust blocker: `Error: compact recording overlay state was not observed: []` at `Tests/computer_use_macos.mjs:259`. Smallest repair: restore the fresh Rust bundle's real Fn-down → compact-overlay transition (or correct only the release-bundle launch/hotkey wiring), then rerun this canary.
- Swift journey: extracted fresh `build/VoicePasteFn-Swift-2026-07-23-archive.app` to `/tmp/voicepaste-swift-canary.GnAB6g/...`, spawned it, set TextEdit element 2, injected Fn down/up. Compact recording overlay was observed with bounds `58x38`, layer 3, onscreen, owner `VoicePasteFn`; exact expected transcription text was not observed in TextEdit within 20 seconds, so exact AX paste failed.
- Swift identity: app PID `52481`, executable SHA-256 `9d2ed77d982e784f3cb2d54a462aedc462b60ca626127ba148a64e4ed261cbbe`, bundle ID `com.bezrabotnyi.voicepastefn`. Durable receipt and screenshot: `.agents/evidence/computer-use/1786273927628-swift/evidence.json`, `.agents/evidence/computer-use/1786273927628-swift/recording.png`.
- Swift blocker: `Error: paste result not observed; element=2` at `Tests/computer_use_macos.mjs:121`; AX remained unchanged after the overlay. Smallest repair: restore the fresh Swift baseline's fixture transcription → result insertion/paste path, then rerun the same canary.
- Verdict: `CHANGES_REQUIRED`. Acceptance proof is incomplete for both implementations; Rust fails at overlay, Swift reaches overlay but fails exact AX paste. No PASS claim.
