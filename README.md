<p align="center">
  <img src="src-tauri/icons/icon.svg" alt="Snaky logo" width="96" height="96" />
</p>

<h1 align="center">Snaky</h1>

<p align="center">
  A small Snake companion for macOS while Codex or Claude Code works.
</p>

<p align="center">
  <img src="docs/screenshot.png" alt="Snaky game popup with Zen mode enabled" width="420" />
</p>

Submitting a prompt shows a tiny snake in the corner with an **⌥ ⇧ S** badge. Press **Option + Shift + S** (Alt + Shift + S), or click the snake, to open the compact translucent game popup. It never opens the full game automatically. **Esc** collapses it back to the corner. Move with the arrow keys or WASD (physical positions, so any keyboard layout works); **Space** pauses or restarts; **R** restarts at any time. The **Zen** toggle in the footer wraps the walls and trims your tail on bites instead of ending the game; your longest tail is kept as the record. Hook settings live behind the gear icon.

The game steps aside when the agent finishes, is interrupted, or actually needs your input. Codex’s automatic approval review does not count as a request for your attention. A read-only observer uses a compatible running Codex app-server proxy to confirm `waitingOnApproval`; without that connection, it does not guess whether approval is needed. Blocking `request_user_input` hooks still signal questions. Claude Code uses the `permission_prompt` notification and `AskUserQuestion` hooks, rather than the predecision `PermissionRequest` event.

## Install locally

Requires macOS, Node.js, and Rust (`cargo`).

```sh
npm install
npm run tauri build
rm -rf /Applications/Snaky.app
cp -R src-tauri/target/release/bundle/macos/Snaky.app /Applications/
/Applications/Snaky.app/Contents/MacOS/agent-snake --install-integrations
```

You can also open `/Applications/Snaky.app` and use the gear menu’s **Connect / repair** button. When updating an existing installation, quit any running `agent-snake` background process (`pkill -x agent-snake`) and restart active Codex or Claude Code terminal sessions so they pick up the updated hooks and binary. Codex may ask you to review and trust the global hooks.

The installer updates `~/.codex/hooks.json` and `~/.claude/settings.json`, preserves unrelated settings and hook commands, and creates a one-time `.agent-snake.backup` beside each existing file. **Disconnect** removes only Snaky’s marked hooks. The hook bridge is bundled in the application; users do not need Node.js at runtime.

## Develop

```sh
npm run tauri dev
```

Run checks with:

```sh
npm test
npm run build
cargo test --manifest-path src-tauri/Cargo.toml
```
