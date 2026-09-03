# Agent Snake

Terminal-style Snake for the time between submitting a coding-agent prompt and getting the result.

## Stack

- Tauri 2 desktop shell (macOS, Windows, Linux)
- TypeScript game engine and ASCII terminal UI
- Rust lifecycle-event listener bound to `127.0.0.1:49271`
- Codex and Claude Code hook adapters

## Develop

```sh
npm install
npm run tauri dev
```

Run tests and production builds with:

```sh
npm test
npm run tauri build
```

## Agent hooks

Project hook configurations are included in `.codex/hooks.json` and `.claude/settings.json`. Review and trust the hooks in your agent before using them.

Submitting a prompt starts or focuses the game. Completion, interruption, and permission requests minimize it so you can return to the agent.

Set `AGENT_SNAKE_BIN` to an installed Agent Snake executable when it is not in a default install location.
