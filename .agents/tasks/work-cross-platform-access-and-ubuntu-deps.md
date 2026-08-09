# Cross-platform access and Ubuntu dependency unblock

Status: in progress
Original request: мак 2.4; Windows найти IP через роутер 2.1; на Ubuntu установить зависимости.

Objective: locate the Mac Mini and Windows host, install the explicitly requested build dependencies on Ubuntu server-100, then rerun the VoicePaste cross-platform production/UI verification where access permits.

Scope: read-only network discovery through the existing router and SSH checks; apt dependency installation on Ubuntu server-100 only; no app deployment, TCC changes, autostart changes, or router configuration changes.

Initial active-minute estimate: 30

## Plan (Russian)

1. Проверить `192.168.2.4` и доступ к роутеру `192.168.2.1`.
2. Найти Windows по DHCP/ARP/соседям роутера.
3. Установить недостающие Ubuntu dependencies на `192.168.2.100`.
4. Повторить Rust build, portable tests и UI canary; сохранить PASS/BLOCKED receipts.

## Progress (English)

- Task opened; host discovery and dependency preflight in progress.

## Evidence (2026-08-09)

- Router `192.168.2.1` was accessed over one-time SSH credentials supplied by the user. DHCP lease identifies Windows as `192.168.2.190`, hostname `Windows-BeyondInfinity`, MAC `64:5d:86:81:8e:0e`. TCP/22 and TCP/3389 are open; key-only SSH as local `user` is rejected, and no Windows password was supplied.
- Mac Mini `192.168.2.4`: host `mac-mini-2012`, macOS 15.7.8, IP confirmed. User/password SSH access works. No Rust toolchain and no VoicePaste app were installed. Swift package tests from a temporary checkout passed 13/13; Swift release executable build passed. Native STT permission probe returned `speech_recognition:false` and exit 1, matching expected permission-gated behavior.
- Ubuntu `192.168.2.100`: installed `libasound2-dev` plus Tauri Linux build dependencies (`libwebkit2gtk-4.1-dev`, `libjavascriptcoregtk-4.1-dev`, `libsoup-3.0-dev`, `libgtk-3-dev`, `libayatana-appindicator3-dev`, `librsvg2-dev`, `libxdo-dev`, and already-present build/SSL packages). Production release Rust build passed after temporary Linux external-bin placeholders were supplied in `/tmp`.
- Ubuntu unit test run reached 76 tests but had 2 failures in existing non-macOS native-STT error expectations: `real_native_stt_surfaces_clean_error_when_helper_missing` and `new_for_test_with_bogus_helper_returns_clean_err` expect macOS helper wording on Linux.
- Ubuntu GUI launch from the release binary reached the real Tauri process and overlay, but global shortcut registration failed with `Unable to find keycode for key: F13`; startup emitted `paste-error`. This is a real Linux hotkey/platform blocker, not a successful paste.

Current classification: Mac Mini Swift PASS / Rust BLOCKED by missing toolchain and app; Windows located / functional test BLOCKED by missing Windows credentials; Ubuntu build PASS after dependencies, UI FAIL at hotkey registration, unit suite PARTIAL (74 passed, 2 platform-assumption failures). No repository code changed in this task.
