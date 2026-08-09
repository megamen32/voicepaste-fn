## 2026-08-09 — Rust/Swift parity Computer Use (completed)

- What slowed or confused L? The app is menu-bar-only and `sky.set_value` did not make TextEdit OS-frontmost; stale bundle/helper identities also caused false UI conclusions.
- Which instruction should change? `AGENTS.md`: require exact app/helper paths and identity receipt in every UI canary; proposed, now encoded in `Tests/computer_use_macos.mjs`.
- Which skill, MCP, or tool is missing? none; `@oai/sky` plus a small CGWindow probe and product lifecycle receipt were sufficient.
- What operation or error repeated? 3 canary identity/overlay failures before the final fresh-bundle PASS; guard with exact workspace paths, PID/hash/codesign receipt, and deterministic target/audio seams.
- State: fixed now
