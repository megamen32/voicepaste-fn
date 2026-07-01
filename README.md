# VoicePaste Fn – Minimal macOS Voice Transcriber

> 📖 [Русская документация](README_RU.md)

**VoicePaste Fn** is a lightweight macOS menu bar application that transcribes voice input to text in real-time using OpenAI's Whisper API or any compatible Whisper endpoint. Hold the `Fn` key, speak, release, and your words appear in the clipboard—ready to paste anywhere.
<img width="305" height="223" alt="image" src="https://github.com/user-attachments/assets/140a5c58-a0c9-48ca-ac16-bc31f55530ee" />

## Features

✨ **One-Key Voice Input**
- Hold `Fn` key (≥0.2s) to record
- Release to transcribe & copy to clipboard
- Automatic paste-ready workflow

📺 **Visual Feedback**
- Overlay window shows recording status (`REC`)
- Real-time transcription preview
- Error notifications and connection status

🌍 **Multi-Language**
- Russian, English, Auto-detect modes
- Configurable via menu bar

⚙️ **Minimal Configuration**
- Works out of the box with embedded defaults
- Environment variables for custom API endpoints
- Menu bar settings for language and preferences

## Quick Start

### Prerequisites
- macOS 13+
- Swift 5.9 or later
- OpenAI API key (or compatible Whisper endpoint)

### Installation

```bash
git clone https://github.com/yourusername/voicepaste-fn.git
cd voicepaste-fn
chmod +x run.sh
./run.sh
```

The app builds to `build/VoicePasteFn.app`. Grant the following permissions when prompted:

```
System Settings → Privacy & Security → Microphone
System Settings → Privacy & Security → Accessibility  
System Settings → Privacy & Security → Input Monitoring
```

After granting permissions, quit the app and run `./run.sh` again.

## Configuration

### ⚠️ Required: Environment Variables

You **must** set these environment variables before running the app:

```bash
export OPENAI_BASE_URL="https://api.openai.com/v1"  # or your self-hosted Whisper server
export OPENAI_API_KEY="sk-your-key-here"            # your API key
export TRANSCRIBE_MODEL="whisper-1"                 # optional, default: whisper-1
```

**Compatible with any Whisper-compatible API endpoint:**
- ✅ OpenAI API: `https://api.openai.com/v1`
- ✅ Self-hosted Whisper servers
- ✅ Third-party Whisper API providers
- ✅ Any OpenAI-compatible API endpoint

**Example with custom server:**
```bash
export OPENAI_BASE_URL="https://your-whisper-server.com/v1"
export OPENAI_API_KEY="your-api-key"
```

### Language Selection

Available in the menu bar:
- **ru** – Russian
- **en** – English  
- **auto** – Auto-detect

### Other Options

- **Realtime Preview** – Shows transcription as it happens
- **Autostart** – Launch VoicePaste on system startup
- **Quit** – Exit the application

## Usage

1. **Launch the app** – Look for the microphone icon or `VP` in the menu bar
2. **Record** – Hold `Fn` key, speak clearly, then release
3. **Paste** – Your transcribed text is automatically copied; use `Cmd+V` anywhere

The overlay window displays:
- Recording indicator (`REC`)
- Live transcription preview (if enabled)
- Errors or connection status

## Project Structure

```
voicepaste-fn/
├── Package.swift              # Swift package manifest
├── README.md                  # English documentation (this file)
├── README_RU.md              # Russian documentation
├── run.sh                     # Build and launch script
├── Sources/
│   └── VoicePasteFn/
│       └── main.swift         # Application source
└── build/
    └── VoicePasteFn.app/      # Built application bundle
```

## Technical Details

**Built with:**
- Swift 5.9
- AppKit (macOS UI framework)
- AVFoundation (audio recording)
- URLSession (API communication)

**Why .app bundle?**
Menu bar applications on macOS are significantly more reliable when distributed as proper `.app` bundles rather than raw CLI executables.

## Permissions

VoicePaste Fn requires the following macOS permissions:
- **Microphone** – To capture audio input
- **Accessibility** – For global `Fn` key monitoring
- **Input Monitoring** – For secure input detection

## Troubleshooting

| Issue | Solution |
|-------|----------|
| Fn key not working | Check Accessibility permissions in System Settings |
| Audio not recorded | Verify Microphone permission is granted |
| Transcription fails | Check API endpoint and key; verify internet connection |
| Menu bar icon missing | Quit app, run `./run.sh` again |

## License

MIT – See [LICENSE](LICENSE) file for details

## Contributing

Pull requests welcome! Please:
1. Fork the repository
2. Create a feature branch (`git checkout -b feature/amazing-feature`)
3. Commit changes (`git commit -m 'Add amazing feature'`)
4. Push to branch (`git push origin feature/amazing-feature`)
5. Open a Pull Request

## Support

For issues, feature requests, or questions, please open a [GitHub Issue](https://github.com/yourusername/voicepaste-fn/issues).

---

**Made with ❤️ for macOS power users**
