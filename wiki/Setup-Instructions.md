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
For the Rust/Tauri app, add `/Applications/VoicePaste.app` under Privacy & Security → Accessibility, then reselect the hotkey or restart VoicePaste. The app shows a visible hotkey error if the CGEvent monitor cannot start.

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

1. Open Settings → Models and click Download beside Parakeet v3.
2. Wait for the sherpa-onnx archive to finish and unpack. The Use model button stays disabled until the model files are present.
3. Install or build a local Parakeet/sherpa runtime for the current OS.
4. In Advanced, enter a command using `{input_path}`, `{output_path}`, `{language}` and `{model_dir}`.
5. Enable Local.

The Rust app downloads the model package but does not silently install a heavyweight platform-specific runtime. A runtime command can write plain text to `{output_path}` or return it on stdout.

## Configure transcription history

Open Settings → History and choose 7, 30 or 90 days, or Forever. Only completed text and metadata are retained; recorded audio is not copied into the history store.
