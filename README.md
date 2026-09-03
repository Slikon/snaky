# Agent Snake

A terminal-style Snake game that opens while Codex or Claude Code is working. The MVP targets macOS.

## How it works

Agent Snake installs marked hooks into `~/.codex/hooks.json` and `~/.claude/settings.json`. A hook starts the app or sends it a local lifecycle event through `127.0.0.1:49271`. The game opens when a prompt starts and minimizes when the turn finishes, is interrupted, or needs attention.

The hook bridge is part of the Rust application binary, so users do not need Node.js. The game and terminal interface remain TypeScript.

## Install locally

```sh
npm install
npm run tauri build
```

Move `src-tauri/target/release/bundle/macos/Agent Snake.app` to `/Applications`, open it once, then select **Install / Repair**. Codex may ask you to review and trust the new global hooks.

The installer preserves unrelated settings and creates a one-time `.agent-snake.backup` beside each existing configuration file. **Uninstall** removes only Agent Snake's marked hooks.

## Develop

```sh
npm run tauri dev
```

Run the lightweight checks with:

```sh
npm test
npm run build
cargo check --manifest-path src-tauri/Cargo.toml
```
