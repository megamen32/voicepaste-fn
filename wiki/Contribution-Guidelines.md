# Contribution Guidelines

## Scope

Contributions may target the Swift/macOS client, the Rust/Tauri cross-platform client, shared protocol/probe/test behavior, or documentation/wiki pages. State which client was tested; a Swift fix does not automatically fix Rust.

## Development workflow

```bash
git switch -c codex/short-description

# Swift
swift build --product voicepaste-fn
swift test

# Rust/Tauri
cd VoicePasteTauri/src-tauri
cargo check
cargo test --lib

# From the repository root: portable suite, including focused-field paste
python3 Tests/run_cross_platform.py
```

For UI changes test the actual lifecycle: hotkey down, recording, hotkey up, processing, success/error, retry and hide.

## Pull requests

State:

1. Which client(s) changed.
2. Root cause and user-visible behavior.
3. Permission, proxy and secret handling.
4. Commands/tests run.
5. Platform-specific limitations.

Do not use `git add -A` in a mixed worktree. Exclude `.env`, generated build directories, graph caches, `.DS_Store` and developer-specific binaries unless a release explicitly requires a target binary.

## Bug reports

Include OS/architecture, client, selected engine/model, activation mode/hotkey, the failing lifecycle stage, and sanitized logs without API keys or private endpoint credentials.
