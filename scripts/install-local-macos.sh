#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
TAURI="$ROOT/VoicePasteTauri/src-tauri"
BUILD_APP="$TAURI/target/release/bundle/macos/VoicePaste.app"
INSTALL_APP="/Applications/VoicePaste.app"

echo "Building VoicePaste $(/usr/libexec/PlistBuddy -c 'Print :CFBundleShortVersionString' "$TAURI/Info.plist" 2>/dev/null || echo release)…"
(cd "$TAURI" && cargo tauri build --bundles app)

test -d "$BUILD_APP"
pkill -TERM -f "$INSTALL_APP/Contents/MacOS/voicepaste" 2>/dev/null || true
pkill -TERM -f "$INSTALL_APP/Contents/MacOS/modifier_monitor" 2>/dev/null || true
sleep 1

ditto --rsrc --extattr --qtn "$BUILD_APP" "$INSTALL_APP"
codesign --force --deep --sign - "$INSTALL_APP"

version=$(/usr/libexec/PlistBuddy -c 'Print :CFBundleShortVersionString' "$INSTALL_APP/Contents/Info.plist")
echo "Installed VoicePaste $version at $INSTALL_APP"

open "$INSTALL_APP"
open 'x-apple.systempreferences:com.apple.preference.security?Privacy_Microphone'
open 'x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility'
echo "Enable VoicePaste in Microphone and Accessibility, then relaunch it."
