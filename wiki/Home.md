# VoicePaste Wiki

VoicePaste has two maintained clients:

- **VoicePaste Fn** — native macOS Swift menu-bar client.
- **VoicePaste** — Rust + Tauri client for macOS, Windows and Ubuntu/Linux.

Both clients use OpenAI-compatible Whisper APIs. The Rust client also supports local Whisper via `whisper-rs`, downloadable Parakeet v3 sherpa-onnx model files, background transcription and a full Settings window.

## Quick links

- [[Architecture Overview]]
- [[Setup Instructions]]
- [[API Documentation]]
- [[Deployment Procedures]]
- [[Coding Standards]]
- [[Contribution Guidelines]]

## Capability matrix

| Capability | Swift Fn | Rust/Tauri |
|---|---:|---:|
| Native macOS menu bar | Yes | Yes |
| Windows / Ubuntu | No | Yes |
| Remote OpenAI-compatible endpoint | Yes | Yes |
| Local Whisper | Optional fallback | Yes |
| Parakeet v3 | No | Downloaded model + configurable local runner |
| Full Settings window | Menu dialogs | General / Models / Remote / Advanced / History / Permissions |
| UI languages | English / Russian / Chinese | English / Russian / Chinese |

## Status indicators

Recording, processing and errors use compact animation/icon indicators without translatable status words. Actual transcript text remains visible for previews and results. Rust errors expose details as a tooltip and provide a retry icon.

## First run

1. Grant microphone permission.
2. Grant Accessibility/Input Monitoring where the selected hotkey requires it.
3. Open Settings and choose Remote, Local or Native.
4. Configure an endpoint/API key, download Whisper or Parakeet, and configure a local Parakeet runner if needed.
5. Choose the transcription language and activation mode.

The first UI launch selects English, Russian or Chinese from the system locale and persists the choice. The language can be changed later in Settings.

## Important paths

- Swift source: `Sources/VoicePasteFn/`
- Swift shared code: `Sources/VoicePasteLib/`
- Swift tests: `Tests/`
- Rust/Tauri source: `VoicePasteTauri/src-tauri/src/`
- Rust/Tauri frontend: `VoicePasteTauri/src/`
- Rust helper sources/tests: `VoicePasteTauri/src-tauri/Sources/`, `Tests/`
- Release artifacts: `artifacts/` (Swift archive and current macOS Rust/Tauri DMG)

## Rust processing behavior

Stopping one recording starts its transcription on a background worker immediately. A second recording can begin while the first one is still being processed; completed text is pasted and written to history independently. History retention is configurable as 7, 30 or 90 days, or forever. Audio files are not stored in history.
