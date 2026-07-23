# Deployment Procedures

## Validation gate

```bash
# Rust/Tauri
cd VoicePasteTauri/src-tauri
cargo check
cargo test --lib
cargo tauri build

# Swift
cd ../..
swift build --product voicepaste-fn
swift test

# Frontend syntax
node --check VoicePasteTauri/src/overlay.js
node --check VoicePasteTauri/src/settings.js
```

Also run `git diff --check` and refresh the graph with `graphify update .` after source changes.

## Swift macOS archive

Build the release product:

```bash
swift build -c release --product voicepaste-fn
```

Create an app bundle with `Contents/MacOS/voicepaste-fn`, `Info.plist`, `PkgInfo` and optional `AppIcon.icns`, then sign and archive it:

```bash
codesign --force --deep --sign - \
  --identifier com.bezrabotnyi.voicepastefn VoicePasteFn.app
ditto -c -k --sequesterRsrc --keepParent \
  VoicePasteFn.app VoicePasteFn.app.zip
```

Do not include `.env`, API keys, Keychain exports or local model files.

## Rust/Tauri packages

```bash
cd VoicePasteTauri/src-tauri
cargo tauri build
```

Tauri writes platform-specific artifacts under `target/release/bundle/`:

- macOS `.app`/DMG;
- Windows MSI/NSIS;
- Linux deb/AppImage/rpm depending on configured targets.

Cross-platform CI must build native helper binaries for the target OS/architecture instead of committing one developer's Mach-O helper into the release.

## Release checklist

- Verify client identities and versions.
- Verify no secret literals or local `.env` files are staged.
- Smoke-test Remote and Local on the target platform.
- Test final macOS bundle permissions.
- Attach the Swift `.app.zip` and Rust/Tauri platform packages to the release.
- Keep `target/`, `.build/`, `build/`, graph caches and logs out of Git.
