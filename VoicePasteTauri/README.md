# VoicePaste

Cross-platform (macOS / Windows / Linux) voice-to-clipboard app built with **Rust + Tauri v2**.

Record audio from your microphone, transcribe via Whisper API (with 3x auto-retry + whisper.cpp local fallback), and paste the result directly to your clipboard.

![Platform](https://img.shields.io/badge/platform-macOS%20%7C%20Windows%20%7C%20Linux-blue)
![License](https://img.shields.io/badge/license-MIT-green)

## Features

- **Global hotkey** — hold or toggle to record (Right Alt by default)
- **Whisper API transcription** — OpenAI-compatible endpoint with 3x auto-retry
- **Local fallback** — whisper.cpp (whisper-rs) for offline transcription when server fails
- **Floating overlay** — always-on-top HUD that follows your cursor showing recording state and transcription preview
- **System tray** — full settings menu with all options
- **Recording queue** — chain multiple recordings
- **Cross-platform** — macOS, Windows, Linux from a single codebase
- **Autostart** — LaunchAgent (macOS), Registry (Windows), XDG (Linux)
- **Configurable** — endpoint, API key, language, model, delays, hotkey, activation mode

## Screenshots

| Recording | Transcribing | Preview |
|-----------|-------------|---------|
| Red dot indicator | Animated waiting | Text preview near cursor |

## Installation

### From DMG (macOS)

1. Download the current `VoicePaste_2.0.0_*` installer from [Releases](../../releases)
2. Drag `VoicePaste.app` to Applications
3. Launch and grant microphone + accessibility permissions

### From source

```bash
# Prerequisites: Rust toolchain, cmake
cargo install tauri-cli --version "^2"

cd VoicePasteTauri/src-tauri
cargo tauri build
```

The built app will be at:
- macOS: `target/release/bundle/macos/VoicePaste.app`
- macOS DMG: `target/release/bundle/dmg/VoicePaste_*.dmg`
- Windows: `target/release/bundle/msi/VoicePaste_*.msi`
- Linux: `target/release/bundle/deb/voicepaste_*.deb`

## Usage

1. **Launch** the app — a tray icon appears in your menu bar
2. **Press and hold** Right Alt (or your configured hotkey) to start recording
3. **Release** to stop and transcribe
4. The transcribed text is automatically pasted at your cursor position

### Toggle mode

In the tray menu, switch activation to **Toggle**:
- First press: start recording
- Second press: stop and transcribe

### Tray menu options

| Option | Description |
|--------|-------------|
| Settings > Endpoint | Whisper API base URL |
| Settings > API Key | Your API key |
| Recording delay | Delay before recording starts (0.2–2.0s) |
| Preview hide delay | How long preview stays visible (0–5s) |
| Language | ru / en / auto |
| Model | Whisper model selection |
| Realtime preview | Adaptive VAD; each completed phrase is sent once and the assembled draft is copied without insertion |
| VAD sensitivity / phrase pause | Tune speech detection and when a phrase is considered complete |
| Autostart | Launch on system startup |
| Hotkey | Choose global hotkey |
| Activation mode | Hold or Toggle |
| Centre overlay | Pin overlay to screen center |
| Wake server | Send silent request before recording |
| Local fallback | Use whisper.cpp on server failure |

## Configuration

Settings are stored as JSON in your platform's app data directory:

- **macOS**: `~/Library/Application Support/com.bezrabotnyi.voicepaste/settings.json`
- **Windows**: `%APPDATA%\com.bezrabotnyi.voicepaste\settings.json`
- **Linux**: `~/.config/com.bezrabotnyi.voicepaste/settings.json`

Environment variables override settings on launch:

| Variable | Description |
|----------|-------------|
| `OPENAI_BASE_URL` | Whisper API endpoint |
| `OPENAI_API_KEY` | API key |
| `TRANSCRIBE_MODEL` | Model name |

## Architecture

```
VoicePasteTauri/
├── src-tauri/
│   ├── src/
│   │   ├── main.rs                  # Entry point
│   │   ├── lib.rs                   # App wiring, commands, hotkey logic
│   │   ├── audio_recorder.rs        # cpal audio capture → WAV
│   │   ├── transcriber.rs           # Whisper API HTTP client
│   │   ├── transcription_service.rs # Retry + fallback orchestration
│   │   ├── local_transcriber.rs     # whisper.cpp local STT
│   │   ├── recording_queue.rs       # State machine (12 tests)
│   │   ├── text_cleaner.rs          # Text cleanup (8 tests)
│   │   ├── overlay.rs               # Floating window + cursor tracking
│   │   ├── tray.rs                  # System tray menu
│   │   ├── hotkey.rs                # Global shortcut manager
│   │   ├── config.rs                # JSON settings persistence
│   │   ├── autostart_manager.rs     # Cross-platform autostart
│   │   ├── pasteboard_typer.rs      # Clipboard + paste simulation
│   │   ├── wake_wav.rs              # Silence WAV generator
│   │   └── models.rs                # Enums (Language, Hotkey, etc.)
│   └── tauri.conf.json
└── src/
    ├── index.html                   # Overlay UI
    ├── overlay.css                  # Dark HUD styling
    └── overlay.js                   # State machine + Tauri events
```

## Development

```bash
cd VoicePasteTauri/src-tauri

# Check compilation
cargo check

# Run tests (30 unit tests)
cargo test

# Run in dev mode
cargo tauri dev

# Production build
cargo tauri build
```

## Tech Stack

- **Rust** — backend language
- **Tauri v2** — cross-platform desktop framework
- **cpal** — cross-platform audio I/O
- **hound** — WAV encoding
- **whisper-rs** — local whisper.cpp STT
- **reqwest** — HTTP client for Whisper API
- **core-graphics** — native cursor position (macOS)

## Translations

- [Русский](README_RU.md)
- [中文](README_CN.md)

## License

MIT
