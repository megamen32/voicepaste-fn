#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "$0")" && pwd)"
cd "$ROOT"

APP_DIR="$ROOT/build/VoicePasteFn.app"
BIN_DIR="$APP_DIR/Contents/MacOS"
RES_DIR="$APP_DIR/Contents/Resources"
BIN_PATH="$BIN_DIR/voicepaste-fn"

# Defaults are embedded in the app. These env vars may override them if needed.
# export OPENAI_BASE_URL="https://example.com/v1"
# export OPENAI_API_KEY="..."
# export TRANSCRIBE_MODEL="whisper-1"

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
</dict>
</plist>
PLIST

printf 'APPL????' > "$APP_DIR/Contents/PkgInfo"

# Relaunch cleanly, so the menu-bar item is owned by a real .app bundle.
osascript -e 'tell application "VoicePasteFn" to quit' >/dev/null 2>&1 || true
open -n "$APP_DIR"

echo "VoicePasteFn launched as app: $APP_DIR"
echo "Look for the microphone icon / VP in the macOS menu bar."
echo "If permissions were requested, grant them to VoicePasteFn.app, then run ./run.sh again."
