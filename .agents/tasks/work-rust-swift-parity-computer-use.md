# Задача: паритет Rust со Swift и автоматическая проверка через Computer Use

Status: work

## Оригинальный запрос

«до сих пор rust хуже чем swift версия. почему? у swift - минималистчиный overlay / swift умеет вставлять а rust только в буфер копирует и не вставляет. Раст - как будто имеет хуже качество перевода и не умеет пользоваться нативным распознанием речи пишет ошибку. но самое важно не построено автоматическое тестирование(не тесты а computer use) !»

## Цель

Установить проверяемые причины функционального отставания Rust от Swift и построить реальный автоматический Computer Use/UI canary, который запускает приложение, инициирует запись, проверяет overlay, распознавание, вставку в активное поле и итоговый текст.

## Бизнес-canary

На чистом тестовом foreground-приложении Rust VoicePaste по горячей клавише показывает компактный overlay, получает текст, копирует его, вставляет через Cmd+V в активное поле и отображает корректное состояние; тот же сценарий сравним со Swift там, где Swift доступен.

## Подтверждённый scope

- сравнение Swift и Rust runtime paths: overlay, paste, transcription/native speech;
- диагностика текущего способа сборки/запуска и реального активного binary/PID;
- Computer Use сценарий и evidence для macOS UI;
- focused red canary до исправлений поведения, если будет принято решение исправлять.

## Явные исключения

- не менять провайдеры, разрешения, секреты или системные настройки без отдельного подтверждения;
- не считать unit-тесты, HTTP health или наличие процесса доказательством UI-поведения;
- не удалять и не откатывать чужие изменения.

## Оценка active minutes (immutable initial)

- optimistic: 30
- likely: 60
- pessimistic: 120

## Первичный план (русский)

1. Сначала получить графовый срез и карту владельцев поведения Rust/Swift.
2. Затем воспроизвести минимальный пользовательский canary через Computer Use и зафиксировать точные точки отказа.
3. Сопоставить отказ с кодом и существующими black-box/installed-helper проверками.
4. Сформировать три плана выравнивания и отдельный обязательный UI-canary слой; до выбора не вносить архитектурные изменения.

## Оценки и evidence

- 2026-08-09: начальная оценка; scope включает несколько архитектурных подсистем и реальный macOS UI, поэтому классифицировано как Full.

## Evidence 2026-08-09

- Graphify: Rust flow проходит через `OverlayManager`/Tauri WebView, `CascadeTranscriber`, внешний Swift `native_stt` и внешний `modifier_monitor`; Swift использует AppKit `RecordingOverlay`, `RetryTranscriber` и собственный `NSPasteboard + CGEvent`.
- Rust source contains real paste path: `pasteboard_typer.rs` calls bundled `modifier_monitor --paste --pid`, and helper performs `NSPasteboard`, target activation and Cmd+V. Therefore clipboard-only symptom is not explained by a missing Rust call; installed-binary/helper/TCC parity remains to be proven.
- Installed `/Applications/VoicePaste.app` exists but no VoicePaste process is running; only the separate Swift build artifact is present.
- Read-only installed helper check: `modifier_monitor --permissions` returned `{"accessibility":true,"input_monitoring":true,"microphone":true,"speech_recognition":false}`. This directly explains why Native STT reports an error on this machine until speech authorization is addressed.
- `NativeSttService::is_available()` returns `true` on every macOS host even when speech permission/helper readiness is false; Native is therefore admitted into the cascade and fails at runtime instead of being skipped or surfaced as a setup state.
- Existing `Tests/blackbox_paste.py` verifies a standalone `paste_probe` into a Tk field. It does not launch the Rust app, press the real hotkey, observe the overlay, run recording/STT, or verify the app's end-to-end paste. `run_cross_platform.py` can also report UI `SKIP`.
- No Computer Use harness using `@oai/sky` exists in the repository. The requested UI gate is therefore absent.
- Оценка revision 2026-08-09: likely 90 active minutes (trigger: live permission evidence plus missing Computer Use harness and multiple runtime owners); pessimistic 180 if a real authenticated speech/foreground-app run requires manual permission or build recovery.

## Выбранный план

2026-08-09: пользователь выбрал «Максимально идеальный».

### stop_when

Только когда свежий Rust build проходит реальный Computer Use сценарий: hotkey → recording overlay → transcription → compact result/error state → paste в заранее сфокусированное тестовое поле, с сохранёнными screenshot/AX/text evidence и сравнительным Swift baseline.

### abandon_when

Если для живого canary требуется пользовательское подтверждение системного Speech/Accessibility prompt или ручное изменение TCC; в этом случае остановиться перед consequential action и запросить его отдельно.

### forbidden_without_explicit_user_request

Изменение системных разрешений/TCC, смена провайдера или модели, публикация/деплой, удаление артефактов, rollback чужих изменений.

### Технический preview

1. UI-gate: добавить macOS Computer Use runner на `@oai/sky`, foreground test target и evidence bundle; запускать приложение из конкретного свежего `.app`, проверять PID/bundle/helper hashes.
2. Paste parity: один канонический macOS path `NSPasteboard → activate captured target PID → Cmd+V`; сделать readiness/error observable и проверить installed helper, не маскируя clipboard-only success.
3. Native STT parity: readiness должен учитывать helper, Speech authorization и locale; `notDetermined/denied/unavailable` — явное состояние setup/fallback, а не runtime surprise. Сохранить on-device preference и error codes.
4. Transcription parity: зафиксировать engine order/model/language in diagnostics, сравнить одинаковый WAV и одинаковую модель Swift/Rust; не объявлять качество хуже без controlled fixture.
5. Overlay parity: оставить Tauri только если свежий Computer Use baseline докажет компактность; иначе свести recording state к одному минимальному native-like surface, не смешивая preview/error layout.
6. Verification: focused red UI canary before each behavior fix; Rust/Swift tests; graphify update; independent Reviewer; Critic; fresh Tester in `only-new` mode; no commit until the real canary passes.

### Call-stack tree

`Computer Use (@oai/sky)` → foreground target + real hotkey → `hotkey.rs` → `start_recording` → `audio_recorder`/`OverlayManager` → `stop_and_transcribe` → `CascadeTranscriber` → `NativeSttService`/remote/local → `PasteboardTyper` → bundled `modifier_monitor --paste --pid` → target field.

### File-tree preview

- add `Tests/computer_use_macos.*` or equivalent runner + evidence schema;
- modify `VoicePasteTauri/src-tauri/src/pasteboard_typer.rs`, `native_stt.rs`, `lib.rs`, `overlay.rs` only where red canary proves a defect;
- modify `VoicePasteTauri/src-tauri/Sources/NativeSTT*` only for confirmed bridge/readiness issues;
- add focused fixtures under `Tests/` and document the real UI gate in `README_RU.md`/`README.md`;
- do not touch unrelated existing dirty paths until ownership and canary impact are reviewed.

### Execution graph

`UI-gate research` ∥ `paste/STT source contract review` → `red Computer Use canary` → `paste/STT/overlay implementation slices` → `focused checks` → `Reviewer` → `Critic` → `Tester only-new` → commit.

### Budget

Selected scope: 60 / 120 / 240 active minutes; relative cost high; critical uncertainty is macOS UI/TCC and the existing dirty worktree. Parallel research lanes reduce wall-clock but not total quota.
