# Snaky

A small Snake companion for macOS while Codex or Claude Code works.

Submitting a prompt shows a tiny snake in the corner with an **⌥ ⇧ S** badge. Press **Option + Shift + S** (Alt + Shift + S), or click the snake, to open the compact translucent game popup. It never opens the full game automatically. **Esc** collapses it back to the corner. Move with the arrow keys or WASD; Space pauses or restarts. Hook settings live behind the gear icon.

The game steps aside when the agent finishes, is interrupted, or actually needs your input. Codex’s automatic approval review does not count as a request for your attention. A read-only observer uses a compatible running Codex app-server proxy to confirm `waitingOnApproval`; without that connection, it does not guess whether approval is needed. Blocking `request_user_input` hooks still signal questions. Claude Code uses the `permission_prompt` notification and `AskUserQuestion` hooks, rather than the predecision `PermissionRequest` event.

## Install locally

```sh
npm install
npm run tauri build
```

Move `src-tauri/target/release/bundle/macos/Snaky.app` to `/Applications`, open it once, then use the gear menu’s **Connect / repair**. Run Connect / repair when updating from an earlier version to replace the old attention hooks. Codex may ask you to review and trust the global hooks.

The installer updates `~/.codex/hooks.json` and `~/.claude/settings.json`, preserves unrelated settings and hook commands, and creates a one-time `.agent-snake.backup` beside each existing file. **Disconnect** removes only Snaky’s marked hooks. The hook bridge is bundled in the application; users do not need Node.js.

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
