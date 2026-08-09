# Explorer lane: Computer Use canary design

Role: Explorer
Objective: inspect the committed VoicePaste checkout and design the smallest real macOS Computer Use canary using the available @oai/sky/node_repl surface.
Original goal: Rust VoicePaste must reach Swift parity and have a real UI canary.
Owned paths: Tests/, README.md, README_RU.md, run/build entrypoints; read-only.
Excluded: do not edit source, do not change permissions/TCC, do not launch or modify external apps, do not commit.
Acceptance proof: report exact runner entrypoint, app launch/identity strategy, target-field strategy, observed blockers, and evidence schema.
Stop conditions: if the runner cannot be made safe/reproducible without user permission, report the blocker and stop.
Budget: 20 / 35 / 60 active minutes; low-medium relative cost.

## Evidence (Explorer, 2026-08-09)

- Baseline checkout is `ac14628 chore: checkpoint existing VoicePaste parity work`.
- `@oai/sky` is available through the Node REPL: `sky.target === "mac"`; `sky.list_apps()` returned 25 apps and a running `VoicePaste` entry with id `com.bezrabotnyi.voicepaste`.
- App identity is ambiguous by bundle id: `/Applications/VoicePaste.app`, `VoicePasteTauri/src-tauri/target/debug/bundle/macos/VoicePaste.app`, and `VoicePasteTauri/src-tauri/target/release/bundle/macos/VoicePaste.app` all declare `com.bezrabotnyi.voicepaste`; the committed Tauri config declares the same identifier at `VoicePasteTauri/src-tauri/tauri.conf.json:3-5`. The Swift app is distinct: `build/VoicePasteFn.app` declares `com.bezrabotnyi.voicepastefn` and `LSUIElement=true` in `run.sh:12-15,46-75`.
- `sky.get_app_state({app:"com.bezrabotnyi.voicepaste", disableDiff:true})` failed with an ambiguity error. Retrying `/Applications/VoicePaste.app` and display name `VoicePaste` reached the Computer Use timeout (`-10005`), consistent with a menu-bar-only app whose normal UI is an `NSStatusItem`, not a document window (`Sources/VoicePasteFn/VoicePasteApp+Menu.swift:9-25`; Tauri overlay is initially invisible at `tauri.conf.json:15-27`). This is an observed blocker, not a permission/TCC diagnosis.
- Exact existing test runner: `python3 Tests/run_cross_platform.py` (`README.md:102-122`; implementation `Tests/run_cross_platform.py:38-81`). It runs Rust lib tests, `Tests/blackbox_models.py --all`, Swift tests on Darwin, and `Tests/blackbox_paste.py` unless `--skip-ui` is given.
- Existing real focused-field canary is `python3 Tests/blackbox_paste.py`. It creates a topmost Tk `Entry` titled `VoicePaste paste target`, writes a `ready` marker after focusing it, launches the production Rust `paste_probe` with a unique `--text`, waits up to 25 seconds, and asserts exact JSON `{ok:true,value:expected}` (`Tests/blackbox_paste.py:27-67,108-185`). On macOS it optionally uses `/Applications/VoicePaste.app/Contents/MacOS/modifier_monitor` and the frontmost PID (`Tests/blackbox_paste.py:141-157`). This is the best existing target-field fixture and is deterministic because it uses a unique nonce and an explicit result file.
- `run.sh` is the Swift build/launch entrypoint, but it is not safe for this Explorer lane: it rebuilds/removes `build/VoicePasteFn.app`, signs it, quits `VoicePasteFn`, and launches it (`run.sh:39-43,86-96`). Tauri release output is documented as `VoicePasteTauri/src-tauri/target/release/bundle/macos/VoicePaste.app` (`VoicePasteTauri/README.md:40-49`), but no committed runner launches it.
- Computer Use limitation: `sky.press_key` targets a named app and the skill explicitly says it cannot invoke global shortcuts. VoicePaste's Swift hotkey is a global CGEvent tap (`Sources/VoicePasteFn/VoicePasteApp.swift:75-107`), and hold mode needs a press duration beyond the configured debounce (`Sources/VoicePasteFn/VoicePasteApp.swift:176-214` / README.md:86-87). Therefore a sky-only F13/Fn hold is not a reproducible canary. Toggle mode would avoid hold duration, but changing that setting is external app state and is outside this read-only assignment.

## Result / handoff

Smallest safe canary design for a Worker: keep `Tests/blackbox_paste.py` as the production paste assertion and add a thin Node REPL/sky observer only around the fixture. Start the fixture and production app through an explicitly approved runner; resolve the running target by exact app path (never by the ambiguous bundle id), then re-snapshot accessibility state before each action. Use the fixture's title `VoicePaste paste target` and its single Entry as the target, verify the initial empty value, generate a per-run nonce, and verify the exact final field value plus the existing result JSON. Do not use the Swift `run.sh` in the canary because it destroys/replaces the app bundle and launches/quit apps.

Recommended evidence schema: `{commit, app_path, bundle_id, executable_sha256, target_app, target_title, target_element_role, nonce, initial_value, actions:[{tool,app,key_or_element,timestamp}], result_value, result_json, exit_code, permission_status_observed, blocker}`. Redact API keys and do not persist screenshots or clipboard contents beyond the nonce/result needed for the assertion.

Blocker/stop condition: a fully real Computer Use canary cannot be proven safe and reproducible from this checkout alone without an explicit Worker/user-approved launch boundary and a dedicated global-key injection mechanism. `@oai/sky` can inspect/read and interact with a focused field, but its app-scoped `press_key` is not sufficient evidence for VoicePaste's global Fn/F13 event-tap path. Highest-value next probe is an approved Worker run against one exact app path plus a controlled toggle-mode configuration, or a purpose-built test-only global key injector that does not alter user permissions/TCC.
