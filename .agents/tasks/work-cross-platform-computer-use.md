# Cross-platform Computer Use verification

Status: in progress
Original request: протестировать VoicePaste на Windows, Mac Mini и Ubuntu, чтобы убедиться, что приложение работает одинаково; пользователь предпочитает тестирование вместо перехвата управления.

Objective: получить отдельные подтверждённые receipts для Rust на Windows, Mac Mini и Ubuntu с реальным foreground-полем, hotkey/recording, transcription fixture и вставкой; проверить Swift на Mac Mini только если он установлен/собран там.

Business canary: после одного полного сценария ожидаемый nonce появляется в активном текстовом поле каждой доступной машины; ошибка не маскируется успешным копированием в буфер.

Confirmed scope: read-only inspection, запуск уже доступных тестов/бинарников и временный loopback fixture; без изменения TCC, автозапуска, сетевых настроек, установленных приложений или пользовательских настроек.

Explicit exclusions: не перехватывать управление текущим Mac, не деплоить, не устанавливать ПО без отдельного разрешения, не считать model-only тест доказательством UI-вставки.

Initial active-minute estimate: 45

## Plan (Russian)

1. Проверить доступность трёх машин и точные артефакты.
2. Запустить общий кроссплатформенный baseline и Computer Use/UI сценарий там, где он доступен.
3. Сохранить отдельные receipts, разделить PASS / PARTIAL / BLOCKED.
4. Не менять код без подтверждённого дефекта; при дефекте создать отдельный todo.

## Progress (English)

- Task opened; remote inventory and platform runner inspection in progress.

## Evidence (2026-08-09)

- Local MacBook Pro: `python3 Tests/run_cross_platform.py --skip-ui` passed: Rust 76 tests, Rust+Swift model-list black box, Swift 13 tests. UI was intentionally skipped to avoid taking control of the user's current Mac.
- Ubuntu `192.168.2.100` / `roomhacker-server-100`: active GNOME/Xorg session, `xdotool` and `xclip` present. A temporary checkout under `/tmp/voicepaste-cross-platform` could not compile production Rust because `alsa-sys` requires missing system package metadata `alsa.pc` (`libasound2-dev`). No package installation was performed.
- Ubuntu `192.168.2.5` / `server-44`: Ubuntu 22.04 with Xorg, but no `xdotool`; no VoicePaste checkout or installed binary was found in the inspected paths. Not a valid UI PASS.
- Mac Mini candidate `192.168.2.117`: ping and SSH timed out; no test run.
- Windows candidate `192.168.1.100`: host responds and TCP/3389 is open, but SSH is refused and no RDP client/credential channel is available in this session; no test run.

Current classification: local baseline PASS; Ubuntu PARTIAL/BLOCKED by missing ALSA development runtime; Mac Mini BLOCKED by unreachable host; Windows BLOCKED by RDP-only access without an available client/session. No code defect has been inferred from these access blockers.
