# VoicePaste Fn – Minimal macOS Voice Transcriber

> 📖 [Русская документация](README_RU.md)

**VoicePaste Fn** is a lightweight macOS menu bar application that transcribes voice input to text using OpenAI's Whisper API or any compatible Whisper endpoint. Hold a hotkey, speak, release — your words land in the clipboard and paste automatically into the focused app.

## Features

### Dictation
- **Configurable hotkey**: Fn (Globe), Right ⌥/⌃/⌘/⇧, Caps Lock, F13/F14/F15.
- **Hold or Toggle activation**: press-and-release (default) or press-on/press-off.
- **Recording delay** (0.10 – 2.00 s): debounce so accidental presses don't trigger.

### Live feedback
- Floating overlay near the cursor, or **centred on screen** via toggle.
- Real-time transcription preview (toggle in menu bar).
- Retry overlay if the request fails — click ↩ to retranscribe the same audio.

### Whisper endpoint
- Endpoint + API key editable from menu bar → **Settings ▶**.
- Stored in **macOS Keychain** (encrypted, only this app can read).
- `OPENAI_BASE_URL` / `OPENAI_API_KEY` env vars still override for shell testing.
- **Wake server on dictation start**: POST a 1-second silence clip to `/audio/transcriptions` so a cold-loaded model is hot before the real recording lands. Failure-tolerant.

### Text cleanup
- Auto-strip common subtitle-channel boilerplate at the end of transcripts: «Продолжение следует», «Thanks for watching!?», «Субтитры сделал DimaTorzok», «Subtitles by DimaTorzok», «to be continued». Optional trailing punctuation is tolerated.

### Compatibility
- OpenAI (`https://api.openai.com/v1`)
- Self-hosted Whisper servers
- Any OpenAI-compatible API endpoint

## Quick Start

### Prerequisites
- macOS 13+
- Swift 5.9+
- OpenAI API key (or any compatible Whisper endpoint)

### Install

```bash
git clone https://github.com/yourusername/voicepaste-fn.git
cd voicepaste-fn
chmod +x run.sh
./run.sh
```

The bundle is at `build/VoicePasteFn.app`. macOS will ask for these permissions on first launch (allow once and they're cached for this ad-hoc-signed bundle):

```
System Settings → Privacy & Security → Microphone
System Settings → Privacy & Security → Accessibility
```

Then click the mic icon in the menu bar → **Settings ▶ → API Key ▶ → Edit…** and paste your key. macOS asks for Keychain permission the first time (once granted, never again).

## Menu Bar

Everything is configurable from the menu bar — no config file editing required.

```
VoicePaste Fn
─────────────
Settings ▶
   Endpoint:  api.openai.com
   API Key:   sk-•••1234 (24)
─────────────
Recording delay: 0.20s   ▶  [0.10 … 2.00 s]
Preview hide:    0.80s   ▶  [Manual / 0.4 … 5.0 s]
Language:        ru       ▶
Model:           auto     ▶
Realtime preview           (toggle)
Realtime every: 5.00s    ▶  [1 … 30 s, only meaningful when preview is on]
Autostart                 (toggle)
─────────────
Hotkey:     Fn (Globe)   ▶   [Fn / Right ⌥ ⌃ ⌘ ⇧ / Caps / F13 F14 F15]
Activation: Hold         ▶   [Hold / Toggle]
Centre overlay on screen (toggle — show overlay centred vs near the cursor)
Wake server on dictation start (toggle — POST silence-clip warm-up)
─────────────
Permissions: ✓ Mic  ✓ Accessibility
Quit
```

### Notes on hotkey changes
Changing the hotkey in the menu bar takes effect on the **next launch**. The event-tap is set up once at start because recreating it for every key change adds state-machine complexity for little benefit. Activation mode (Hold / Toggle) and all other settings apply immediately.

## Configuration

### Env vars (override UserDefaults / Keychain for a single launch)

```bash
export OPENAI_BASE_URL="https://api.openai.com/v1"
export OPENAI_API_KEY="sk-your-key-here"
export TRANSCRIBE_MODEL="whisper-1"   # default
./run.sh
```

Useful for shell testing without touching saved credentials. Env vars win over UserDefaults/Keychain for that launch only.

## Cross-platform black-box tests

The model-list contract is tested through a real local HTTP server and a
separate probe process:

```bash
python3 Tests/blackbox_models.py --all
```

The Rust probe runs on Windows, macOS, and Ubuntu. The Swift probe runs on
macOS and is skipped automatically on the other platforms. To run one
implementation, use `--implementation rust` or `--implementation swift`.

The current baseline is intentionally red for Rust: Swift passes the sorted
model-list contract, while Rust exposes its current unsorted response.

### Persisted (UserDefaults + Keychain)

Stored in `~/Library/Preferences/com.bezrabotnyi.voicepastefn.plist` and the macOS Keychain (Generic Password, service `com.bezrabotnyi.voicepastefn`, account `openai_api_key`). Edit via menu bar Settings.

## Project Structure

```
voicepaste-fn/
├── Package.swift
├── README.md                # English
├── README_RU.md             # Russian
├── LICENSE
├── run.sh                   # Build + ad-hoc sign + launch
├── AppIcon.icns             # Bundled icon
├── Sources/
│   └── VoicePasteFn/
│       ├── main.swift       # Recording/bootstrap helpers
│       ├── VoicePasteApp.swift
│       └── RecordingOverlay.swift
├── build/
│   └── VoicePasteFn.app/
└── ...
```

## Troubleshooting

| Issue | Solution |
|-------|----------|
| Hotkey not firing | macOS Settings → Privacy & Security → Accessibility → allow VoicePasteFn |
| API key prompt doesn't appear | macOS Settings → Passwords → check Keychain Access for VoicePasteFn |
| First transcription is slow / times out | Either lower Wake-server interval (it already fires on every start), or pre-warm via the menu: Settings ▶ Endpoint ▶ Edit, Cancel, Save (forces one idle round-trip) |
| Overlay is in the way | Toggle "Centre overlay on screen" or move cursor before pressing the hotkey |

## Releases

Pre-built `.app.zip` bundles are published on the GitHub Releases page. The bundle is ad-hoc-signed with a stable identifier (`com.bezrabotnyi.voicepastefn`) so macOS TCC keeps Microphone + Accessibility permissions across reinstalls.

The current development branch also keeps the selected macOS artifacts in [`artifacts/`](artifacts/): the latest Swift archive, the Rust/Tauri DMG, and the two bundled Swift helper binaries. Checksums are in [`artifacts/SHA256SUMS.txt`](artifacts/SHA256SUMS.txt).

```bash
# Download & install a release (example for v0.3.0):
curl -L https://github.com/yourusername/voicepaste-fn/releases/download/v0.3.0/VoicePasteFn.app.zip \
    -o vp.zip
unzip vp.zip
mv VoicePasteFn.app /Applications/
open /Applications/VoicePasteFn.app
```

## Permissions

VoicePasteFn needs:
- **Microphone** – for the recording.
- **Accessibility** – for the global hotkey event tap.
- **Keychain Access** – once, on the first save of an API key.

## License

MIT — see [LICENSE](LICENSE).

## Contributing

PRs welcome. Swift remains the native macOS client; Rust/Tauri is the cross-platform client with local Whisper/Parakeet support. Tests live beside each client and should cover both model availability and background recording behavior.
