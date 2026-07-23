# Setup Instructions

## Rust/Tauri

Prerequisites: Rust stable, Cargo, CMake, Tauri CLI v2, and the WebKitGTK/ALSA development packages required by Linux.

```bash
cd VoicePasteTauri/src-tauri
cargo check
cargo tauri dev
```

The Rust client targets macOS, Windows and Ubuntu/Linux. Native speech is platform-dependent; Remote and Local are the portable paths.

## Swift

Prerequisites: macOS 13 or newer and Swift 5.9/Xcode 15+.

```bash
swift build --product voicepaste-fn
swift test
swift run voicepaste-fn
```

The Swift client is macOS-only and uses AppKit, AVFoundation and the macOS event tap.

## First-run permissions

- Microphone — audio capture.
- Accessibility/Input Monitoring — global Fn/modifier monitoring and paste.
- Speech Recognition — only for the Apple Speech fallback.

If permission was denied, enable it for the exact app/bundle being run in System Settings.

## Configure Remote

1. Open tray → Settings → Remote.
2. Select OpenAI, OpenRouter or Custom.
3. Enter endpoint, model and API key.
4. Use Refresh to query `/models` when supported.

## Configure Local Whisper

1. Open Settings → Models.
2. Select Whisper base.
3. Click Download.
4. Enable Local in the recognition engine list.

`WHISPER_MODEL_PATH` can provide a custom whisper.cpp model.

## Configure Parakeet v3

1. Install a local Parakeet/sherpa runtime.
2. Select Parakeet v3 in Settings → Models.
3. In Advanced, enter a command using `{input_path}`, `{output_path}` and `{language}`.
4. Enable Local.

The Rust app does not silently install a heavyweight ML runtime; the model page is linked from Settings.
