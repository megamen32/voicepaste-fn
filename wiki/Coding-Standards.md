# Coding Standards

## General

- Prefer focused modules over monolithic files.
- If a code file exceeds 800 lines, split it by responsibility before adding more behavior.
- Preserve user settings and unrelated working providers.
- Never print API keys, proxy passwords or raw secret environment values.
- Clear recorder, queue, timers and overlay state on every stop/error path.

## Rust

- Keep persisted config backward-compatible with serde defaults.
- Put Tauri commands in focused modules.
- Keep the tray limited to high-frequency controls; provider credentials, permissions and model management belong in Settings.
- Use integration-oriented tests for provider and lifecycle boundaries.
- Keep `cargo check` and `cargo test --lib` green and preserve Windows/Linux `cfg` guards.

## Swift

- Keep native macOS behavior in Swift; shared pure logic belongs in `VoicePasteLib`.
- Use AppKit icons/animation for lifecycle status, not translatable status words.
- Fn/Globe handling must support modifier flags and keycode events.
- Always clean up recorder, queue, timers and overlay state on stop/error.

## Frontend

- Localize visible Settings copy through the shared locale mechanism.
- Lifecycle overlays are visual-only; transcript content may remain textual.
- Run `node --check` on every changed JavaScript file.

## Providers

- Use stable ids such as `whisper-base` and `parakeet-v3`.
- Do not invent remote model aliases; query the provider catalog when available.
- Keep system and environment proxy behavior intact.
