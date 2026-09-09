import { listen } from "@tauri-apps/api/event";
import { invoke, isTauri } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { Direction, SnakeGame } from "./game";
import "./style.css";

interface Snapshot { working: number; attention: boolean; source: string; gameVisible: boolean; shortcutAvailable: boolean }
interface IntegrationState { installed: boolean }
interface IntegrationReport { codex: IntegrationState; claude: IntegrationState }
const native = isTauri();
const launcher = new URLSearchParams(location.search).has("launcher");
const mac = /Mac/.test(navigator.platform);
const shortcut = mac ? "⌥ ⇧ S" : "Alt ⇧ S";
const shortcutWords = mac ? "Option–Shift–S" : "Alt–Shift–S";
let state: Snapshot = { working: 0, attention: false, source: "agent", gameVisible: !launcher, shortcutAvailable: true };
const pet = `<svg class="snake-pet" viewBox="0 0 80 80" aria-hidden="true"><defs><linearGradient id="skin" x1="0" y1="0" x2="1" y2="1"><stop stop-color="#d5d0ff"/><stop offset="1" stop-color="#a49ce8"/></linearGradient></defs><ellipse class="pet-shadow" cx="40" cy="68" rx="24" ry="4" fill="#29223c" opacity=".13"/><g class="pet-body"><path d="M20 55c-12 0-12-17 0-17h22c12 0 13 17 1 17H31" fill="none" stroke="#625a9a" stroke-width="17" stroke-linecap="round"/><path d="M20 53c-12 0-12-17 0-17h22c12 0 13 17 1 17H31" fill="none" stroke="url(#skin)" stroke-width="14" stroke-linecap="round"/><path d="M42 38V27" stroke="#625a9a" stroke-width="22" stroke-linecap="round"/><rect x="28" y="12" width="34" height="29" rx="14" fill="url(#skin)" stroke="#625a9a" stroke-width="2"/><g class="pet-eyes" fill="#342c50"><ellipse cx="41" cy="24" rx="2.5" ry="3.5"/><ellipse cx="54" cy="24" rx="2.5" ry="3.5"/></g><path d="M44 33q4 3 8 0" fill="none" stroke="#625a9a" stroke-width="1.8" stroke-linecap="round"/></g></svg>`;
const root = document.querySelector<HTMLDivElement>("#app")!;
const command = async (name: string): Promise<void> => { if (native) await invoke(name); };

if (launcher) {
  document.body.classList.add("launcher-mode");
  root.innerHTML = `<div class="companion"><button id="pet" class="pet-button" type="button">${pet}<kbd id="shortcut">${shortcut}</kbd><span id="attention-dot" class="attention-dot" hidden></span></button><button id="dismiss" class="dismiss" aria-label="Hide snake until the next prompt" title="Hide until next prompt">×</button></div>`;
  document.querySelector("#pet")!.addEventListener("click", () => void command("open_game"));
  document.querySelector("#dismiss")!.addEventListener("click", () => void command("dismiss_companion"));
} else {
  root.innerHTML = `<main class="popup" aria-label="Snaky">
    <header><div class="identity" id="drag-handle"><span class="mini-snake">${pet}</span><div><h1>Snaky</h1><p id="status" aria-live="polite">A little breather</p></div></div><span class="score" aria-label="Score"><span id="score">0</span></span><button id="settings-toggle" class="icon-button" aria-label="Settings" aria-expanded="false" title="Settings">⚙</button><button id="close" class="icon-button" aria-label="Tuck away (Escape)" title="Tuck away · Esc">×</button></header>
    <section class="board-wrap"><canvas id="board" tabindex="0" width="720" height="720" aria-label="Snake game board"></canvas><div id="message" class="message"><strong id="message-title"></strong><span id="message-help"></span></div></section>
    <footer><span><kbd>↑ ↓ ← →</kbd> move</span><span><kbd>Space</kbd> pause</span><button id="restart" title="Restart · R" aria-label="Restart game">↻</button></footer>
    <section id="settings" class="settings" hidden aria-label="Settings"><div class="settings-heading"><h2>Make yourself at home</h2><button id="settings-close" class="icon-button" aria-label="Close settings">×</button></div><p>The little snake appears while your agent works. Open it whenever you feel like a break.</p><div class="setting-row"><span>Open / tuck away</span><kbd id="settings-shortcut">${shortcut}</kbd></div><div class="setting-row"><span>Codex</span><span id="codex-state">Checking…</span></div><div class="setting-row"><span>Claude Code</span><span id="claude-state">Checking…</span></div><p id="integration-message" role="status"></p><div class="settings-actions"><button id="install-hooks">Connect / repair</button><button id="remove-hooks">Disconnect</button></div></section>
  </main>`;
}

const game = new SnakeGame(20, 20);
let settingsOpen = false;
function render(): void {
  if (launcher) {
    const button = document.querySelector<HTMLButtonElement>("#pet")!;
    const text = state.attention ? "Your agent needs you" : `${state.source === "claude" ? "Claude" : "Codex"} is working`;
    button.title = `${text} · ${state.shortcutAvailable ? shortcutWords : "Click the snake"} to open Snake`;
    button.setAttribute("aria-label", button.title);
    document.querySelector("#shortcut")!.textContent = state.shortcutAvailable ? shortcut : "Click";
    (document.querySelector("#attention-dot") as HTMLElement).hidden = !state.attention;
    return;
  }
  document.querySelector("#score")!.textContent = String(game.score);
  document.querySelector("#status")!.textContent = state.attention ? "Your agent needs you" : state.working ? `${state.source === "claude" ? "Claude" : "Codex"} is working${state.working > 1 ? ` · ${state.working} tasks` : ""}` : "A little breather";
  document.querySelector("#settings-shortcut")!.textContent = state.shortcutAvailable ? shortcut : "Click the snake";
  const labels: Record<string, [string, string]> = {
    waiting: ["A little breather.", "Arrow keys or Space to begin"],
    paused: ["Take your time.", "Space to continue"],
    "game-over": ["One more round?", "Space or R to start again"],
    attention: ["Your agent needs you.", "Esc to return"],
    finished: ["Back to it.", "Your agent has finished"],
  };
  const label = labels[game.phase];
  (document.querySelector("#message") as HTMLElement).hidden = !label;
  if (label) {
    document.querySelector("#message-title")!.textContent = label[0];
    document.querySelector("#message-help")!.textContent = label[1];
  }
  drawBoard();
}
function drawBoard(): void {
  const canvas = document.querySelector<HTMLCanvasElement>("#board")!;
  const ctx = canvas.getContext("2d")!;
  const cell = canvas.width / game.columns;
  ctx.clearRect(0, 0, canvas.width, canvas.height);
  ctx.fillStyle = "#d8d7de";
  for (let y = 0; y < game.rows; y++) for (let x = 0; x < game.columns; x++) {
    ctx.beginPath(); ctx.arc((x + .5) * cell, (y + .5) * cell, 1.2, 0, Math.PI * 2); ctx.fill();
  }
  ctx.fillStyle = "#df9877";
  ctx.beginPath(); ctx.arc((game.food.x + .5) * cell, (game.food.y + .5) * cell, cell * .27, 0, Math.PI * 2); ctx.fill();
  game.snake.forEach((part, i) => {
    ctx.fillStyle = i === 0 ? "#756aaa" : "#b4a9d9";
    ctx.beginPath();ctx.roundRect(part.x * cell + 2, part.y * cell + 2, cell - 4, cell - 4, i === 0 ? 11 : 8);ctx.fill();
    if (i === 0) {
      const horizontal = game.direction === "left" || game.direction === "right";
      const sign = game.direction === "left" || game.direction === "up" ? -.16 : .16;
      ctx.fillStyle = "#fff";
      for (const side of [-.17, .17]) {ctx.beginPath();ctx.arc((part.x + .5 + (horizontal ? sign : side)) * cell, (part.y + .5 + (horizontal ? side : sign)) * cell, 2.5, 0, Math.PI * 2);ctx.fill();}
    }
  });
}
function setSettings(open: boolean): void {
  settingsOpen = open;
  (document.querySelector("#settings") as HTMLElement).hidden = !open;
  document.querySelector("#settings-toggle")!.setAttribute("aria-expanded", String(open));
  if (open && game.phase === "running") game.phase = "paused";
  render();
  if (!open) document.querySelector<HTMLCanvasElement>("#board")!.focus();
}
async function integrations(commandName = "integration_status"): Promise<void> {
  const message = document.querySelector("#integration-message")!;
  const buttons = document.querySelectorAll<HTMLButtonElement>(".settings-actions button");
  buttons.forEach(button => button.disabled = true);
  try {
    if (!native) { message.textContent = "Desktop preview · Connect agents in the installed app."; return; }
    const report = await invoke<IntegrationReport>(commandName);
    document.querySelector("#codex-state")!.textContent = report.codex.installed ? "Connected" : "Not connected";
    document.querySelector("#claude-state")!.textContent = report.claude.installed ? "Connected" : "Not connected";
    message.textContent = commandName === "install_integrations" ? "Connected. Restart your agent to load the updated hooks." : "";
  } catch (error) { message.textContent = String(error); }
  finally { buttons.forEach(button => button.disabled = false); }
}
if (!launcher) {
  document.querySelector("#close")!.addEventListener("click", () => void command("tuck_game"));
  document.querySelector("#settings-toggle")!.addEventListener("click", () => setSettings(!settingsOpen));
  document.querySelector("#settings-close")!.addEventListener("click", () => setSettings(false));
  document.querySelector("#restart")!.addEventListener("click", () => { game.reset();render();document.querySelector<HTMLCanvasElement>("#board")!.focus(); });
  document.querySelector("#install-hooks")!.addEventListener("click", () => void integrations("install_integrations"));
  document.querySelector("#remove-hooks")!.addEventListener("click", () => void integrations("uninstall_integrations"));
  document.querySelector("#drag-handle")!.addEventListener("mousedown", event => { if (native && (event as MouseEvent).button === 0) void getCurrentWindow().startDragging(); });
  const directions: Record<string, Direction> = { ArrowUp:"up",w:"up",ArrowDown:"down",s:"down",ArrowLeft:"left",a:"left",ArrowRight:"right",d:"right" };
  let tickTimer: number | undefined;
  function scheduleTick(): void {
    if (tickTimer !== undefined) window.clearTimeout(tickTimer);
    tickTimer = window.setTimeout(tick, Math.max(85, 155 - game.score * 3));
  }
  function tick(): void {
    if (state.gameVisible && !settingsOpen) {game.advance();render();}
    scheduleTick();
  }
  window.addEventListener("keydown", event => {
    if (event.key === "Escape") { if (settingsOpen) setSettings(false); else void command("tuck_game"); return; }
    if (settingsOpen || event.altKey || event.metaKey || event.ctrlKey || (event.target as HTMLElement).closest("button")) return;
    const direction = directions[event.key];
    if (direction) {
      event.preventDefault();
      if (game.phase === "waiting") game.reset();
      if (game.phase === "running" && game.steer(direction)) {game.advance();scheduleTick();}
    }
    else if (event.key === " ") {event.preventDefault();game.togglePause();}
    else if (event.key.toLowerCase() === "r") game.reset();
    render();
  });
  window.addEventListener("blur", () => { if (game.phase === "running") {game.phase = "paused";render();} });
  void integrations();
  tick();
}
function acceptState(next: Snapshot): void {
  if (!launcher) {
    if (!next.gameVisible && game.phase === "running") game.phase = "paused";
    if (next.attention) game.phase = "attention";
    else if (game.phase === "attention") game.phase = "paused";
  }
  const opening = !state.gameVisible && next.gameVisible;
  state = next;render();
  if (!launcher && opening) document.querySelector<HTMLCanvasElement>("#board")!.focus();
}
render();
if (native) {
  await listen<Snapshot>("companion-state", ({payload}) => acceptState(payload));
  acceptState(await invoke<Snapshot>("companion_state"));
  await command("ready");
}
