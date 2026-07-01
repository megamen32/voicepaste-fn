#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "$0")" && pwd)"
cd "$ROOT"

# Note: .env in the project root is still respected if present (env vars set
# before launch override what's saved in UserDefaults / Keychain — useful for
# shell-side testing). No automatic install into ~/.config/ is done; the
# menu bar Endpoint / API Key dialogs are the canonical way to configure.

APP_DIR="$ROOT/build/VoicePasteFn.app"
BIN_DIR="$APP_DIR/Contents/MacOS"
RES_DIR="$APP_DIR/Contents/Resources"
BIN_PATH="$BIN_DIR/voicepaste-fn"

# ⚠️ REQUIRED: Set your Whisper API endpoint before running
# You MUST configure these environment variables:
#
# export OPENAI_BASE_URL="https://api.openai.com/v1"  # or your self-hosted server
# export OPENAI_API_KEY="sk-your-key-here"            # your API key
# export TRANSCRIBE_MODEL="whisper-1"                 # optional, default: whisper-1
#
# Compatible with any OpenAI-compatible Whisper API endpoint.
# See README.md or README_RU.md for more details.

# Install .env to user-level config dir so the .app can launch standalone
# (without sourcing a shell). Done on every build — it's a no-op if unchanged.
ENV_TARGET="$HOME/.config/voicepaste-fn/.env"
if [ -f "$ROOT/.env" ]; then
    mkdir -p "$(dirname "$ENV_TARGET")"
    if ! cmp -s "$ROOT/.env" "$ENV_TARGET" 2>/dev/null; then
        cp "$ROOT/.env" "$ENV_TARGET"
        chmod 600 "$ENV_TARGET"
        echo "Installed $ROOT/.env → $ENV_TARGET"
    fi
fi

swift build -c release

rm -rf "$APP_DIR"
mkdir -p "$BIN_DIR" "$RES_DIR"
cp "$ROOT/.build/release/voicepaste-fn" "$BIN_PATH"
chmod +x "$BIN_PATH"

cat > "$APP_DIR/Contents/Info.plist" <<'PLIST'
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleExecutable</key>
  <string>voicepaste-fn</string>
  <key>CFBundleIdentifier</key>
  <string>com.bezrabotnyi.voicepastefn</string>
  <key>CFBundleName</key>
  <string>VoicePasteFn</string>
  <key>CFBundleDisplayName</key>
  <string>VoicePasteFn</string>
  <key>CFBundlePackageType</key>
  <string>APPL</string>
  <key>CFBundleVersion</key>
  <string>1</string>
  <key>CFBundleShortVersionString</key>
  <string>0.1.0</string>
  <key>LSUIElement</key>
  <true/>
  <key>NSMicrophoneUsageDescription</key>
  <string>VoicePaste records your voice while Fn is held to transcribe it.</string>
  <key>NSHumanReadableCopyright</key>
  <string>VoicePasteFn</string>
  <key>CFBundleIconFile</key>
  <string>AppIcon</string>
</dict>
</plist>
PLIST

printf 'APPL????' > "$APP_DIR/Contents/PkgInfo"

# Bundle the app icon (kept at repo root so rebuilds don't lose it).
if [ -f "$ROOT/AppIcon.icns" ]; then
    cp "$ROOT/AppIcon.icns" "$RES_DIR/AppIcon.icns"
fi

# Sign with a stable ad-hoc identity so macOS TCC keeps microphone/accessibility
# permissions between rebuilds. Without this, every `swift build` produces a
# binary TCC sees as "new", and the user has to re-grant Privacy permissions.
codesign --force --deep --sign - --identifier com.bezrabotnyi.voicepastefn "$APP_DIR"

# Relaunch cleanly, so the menu-bar item is owned by a real .app bundle.
osascript -e 'tell application "VoicePasteFn" to quit' >/dev/null 2>&1 || true
open -n "$APP_DIR"

echo "VoicePasteFn launched as app: $APP_DIR"
echo "Look for the microphone icon / VP in the macOS menu bar."
echo "If permissions were requested, grant them to VoicePasteFn.app, then run ./run.sh again."
