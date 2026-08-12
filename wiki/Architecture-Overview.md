# Architecture Overview

## Two clients, one protocol

```text
                         OpenAI-compatible Whisper API
                                      │
             ┌────────────────────────┴────────────────────────┐
             │                                                 │
      Swift VoicePaste Fn                              Rust VoicePaste
      macOS native client                               Tauri cross-platform
             │                                                 │
      AppKit menu bar + Fn                             tray + Settings webview
      AVAudioRecorder + Swift STT                     cpal + whisper-rs/native STT
```

The clients share product behavior but target different deployment goals. Swift is the native macOS experience; Rust/Tauri is the cross-platform implementation.

## Swift client

- `SettingsModel.swift` — UserDefaults, Keychain, language and activation settings.
- `VoicePasteApp.swift` — application lifecycle and recording flow.
- `VoicePasteApp+Menu.swift` — menu-bar settings and actions.
- `RecordingOverlay.swift` — icon-only status indicator and transcript preview.
- `Sources/VoicePasteLib/` — recorder/transcriber/queue logic shared by probes and tests.

Fn/Globe is handled through both modifier flags and key codes `63`/`179`, because Apple keyboards do not all report Globe/Fn through the same CGEvent family.

## Rust/Tauri client

- `lib.rs` owns Tauri commands and recording/transcription flow.
- `config.rs` persists `AppConfig` and the ordered engine cascade.
- `tray.rs` contains only frequent quick controls.
- `tray_events.rs` handles tray actions.
- `settings_commands.rs` exposes full Settings without returning raw API keys.
- `local_transcriber.rs` owns Whisper model discovery/download and local status.
- `history.rs` persists completed text as JSONL and applies the configured retention window.
- `transcription_service.rs` contains remote, cascade, retry and command-provider adapters.
- `overlay.rs` and `src/overlay.js` render compact animated status states.

Remote, Local and Native can be enabled independently; unavailable tiers are skipped at runtime.

## Local providers

### Whisper base

The built-in local provider uses `whisper-rs` and a `ggml-base.bin` model. Settings downloads the model to the application data directory. `WHISPER_MODEL_PATH` remains supported.

### Parakeet v3

Parakeet is represented by the stable id `parakeet-v3`. Settings downloads and extracts the official sherpa-onnx v3 archive into the app data model directory. The Rust binary does not embed a large NeMo runtime; it runs a configured local command with:

```text
{input_path}  {output_path}  {language}  {model_dir}
```

The command may write plain text to `{output_path}` or stdout. `{model_dir}` points to the downloaded ONNX model directory, keeping the provider portable across macOS, Windows and Linux. The model cannot be selected until the archive is downloaded and validated.

## Background processing and history

`stop_and_transcribe` stops the current recorder, increments the in-flight counter and spawns transcription immediately. The recorder is not blocked by earlier workers. Each completed non-empty result is pasted and appended to `transcription-history.jsonl`; retention pruning runs on append and read.

## Settings and secrets

The Settings webview uses Tauri commands:

- `get_settings` returns masked API-key state only.
- `save_settings` applies partial config updates and re-registers hotkeys.
- `download_local_model` emits progress events.
- `get_history` returns newest-first entries; `clear_history` removes them.
- `refresh_remote_models` queries `/models`.
- `get_permissions` and `open_permissions` expose permission state/actions.

`OPENAI_BASE_URL`, `OPENAI_API_KEY` and `TRANSCRIBE_MODEL` can override compatible runtime settings. reqwest keeps system/environment proxy behavior; the UI shows variable names, never their values.

## Overlay lifecycle

```text
hotkey down → recording indicator → hotkey up → processing spinner
                                      │
                         success → transcript preview → hide
                         error   → error icon + retry icon
```

Lifecycle overlays avoid words so they do not need three translations. Transcript text is still shown when useful.
