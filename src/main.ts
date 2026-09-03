import { listen } from "@tauri-apps/api/event";
import { invoke } from "@tauri-apps/api/core";
import { Direction, SnakeGame } from "./game";
import "./style.css";

interface AgentEvent {
  source: string;
  action: "start" | "stop" | "interrupt" | "attention";
  session_id: string;
  turn_id: string;
}

interface IntegrationState {
  name: string;
  configPath: string;
  detected: boolean;
  installed: boolean;
}

interface IntegrationReport {
  codex: IntegrationState;
  claude: IntegrationState;
  executablePath: string;
}

const game = new SnakeGame();
const activeTurns = new Set<string>();
let agent = "CODEX";
let integrationReport: IntegrationReport | undefined;
let integrationBusy = false;
let integrationError = "";

document.querySelector<HTMLDivElement>("#app")!.innerHTML = `
  <main class="terminal" aria-label="Agent Snake terminal">
    <header>
      <div>AGENT-SNAKE.EXE <span class="dim">v0.3.0</span></div>
      <div class="right">LINK: <span id="agent">CODEX</span></div>
    </header>
    <div class="rule">================================================================================</div>
    <section class="telemetry">
      <span>STATUS: <b id="status">IDLE / WAITING FOR PROMPT</b></span>
      <span>SCORE: <b id="score">00000</b></span>
      <span>JOBS: <b id="jobs">00</b></span>
    </section>
    <section class="integrations" aria-label="Global agent integrations">
      <span>GLOBAL HOOKS: <b id="hook-status">CHECKING...</b></span>
      <span id="hook-details" class="hook-details">CODEX: -- / CLAUDE: --</span>
      <div class="hook-actions">
        <button id="install-hooks" type="button">[I] INSTALL / REPAIR</button>
        <button id="remove-hooks" type="button">[U] UNINSTALL</button>
      </div>
    </section>
    <section class="screen">
      <pre id="board" aria-label="Snake board"></pre>
      <div id="message" class="message"></div>
    </section>
    <div class="rule">--------------------------------------------------------------------------------</div>
    <footer>
      <span>[ARROWS/WASD] MOVE</span>
      <span>[SPACE] PAUSE/RESTART</span>
      <span>[R] RESET</span>
      <span class="cursor">_</span>
    </footer>
  </main>
`;

const board = document.querySelector<HTMLPreElement>("#board")!;
const status = document.querySelector<HTMLElement>("#status")!;
const score = document.querySelector<HTMLElement>("#score")!;
const jobs = document.querySelector<HTMLElement>("#jobs")!;
const agentLabel = document.querySelector<HTMLElement>("#agent")!;
const message = document.querySelector<HTMLElement>("#message")!;
const hookStatus = document.querySelector<HTMLElement>("#hook-status")!;
const hookDetails = document.querySelector<HTMLElement>("#hook-details")!;
const installHooks = document.querySelector<HTMLButtonElement>("#install-hooks")!;
const removeHooks = document.querySelector<HTMLButtonElement>("#remove-hooks")!;

const statusText = (): string => {
  const labels = {
    waiting: "IDLE / WAITING FOR PROMPT",
    running: "AGENT PROCESSING / GAME ACTIVE",
    paused: "PAUSED",
    attention: "ATTENTION REQUIRED BY AGENT",
    finished: "AGENT TURN COMPLETE",
    "game-over": activeTurns.size ? "GAME OVER / AGENT STILL RUNNING" : "GAME OVER",
  };
  return labels[game.phase];
};

const messageText = (): string => {
  if (
    game.phase === "waiting" &&
    integrationReport &&
    (!integrationReport.codex.installed || !integrationReport.claude.installed)
  ) {
    return "[ FIRST RUN ]\nINSTALL GLOBAL HOOKS ABOVE";
  }
  const labels = {
    waiting: "[ READY ]\nSUBMIT A PROMPT OR PRESS SPACE",
    running: "",
    paused: "[ PAUSED ]\nPRESS SPACE TO CONTINUE",
    attention: "[ AGENT NEEDS INPUT ]\nRETURN TO YOUR CODING TERMINAL",
    finished: "[ PROCESS COMPLETE ]\nYOUR AGENT IS READY",
    "game-over": "[ GAME OVER ]\nPRESS SPACE OR R TO REBOOT",
  };
  return labels[game.phase];
};

function renderIntegrations(): void {
  installHooks.disabled = integrationBusy;
  removeHooks.disabled = integrationBusy;
  if (integrationBusy) {
    hookStatus.textContent = "WORKING...";
    return;
  }
  if (integrationError) {
    hookStatus.textContent = "ERROR";
    hookDetails.textContent = integrationError;
    return;
  }
  if (!integrationReport) {
    hookStatus.textContent = "CHECKING...";
    return;
  }
  const { codex, claude } = integrationReport;
  hookStatus.textContent = codex.installed && claude.installed ? "ONLINE" : "SETUP REQUIRED";
  hookDetails.textContent = `CODEX: ${codex.installed ? "ON" : "OFF"} / CLAUDE: ${claude.installed ? "ON" : "OFF"}`;
}

async function refreshIntegrations(): Promise<void> {
  try {
    integrationReport = await invoke<IntegrationReport>("integration_status");
    integrationError = "";
  } catch (error) {
    integrationError = String(error);
  }
  renderIntegrations();
  render();
}

async function updateIntegrations(command: "install_integrations" | "uninstall_integrations"): Promise<void> {
  integrationBusy = true;
  integrationError = "";
  renderIntegrations();
  try {
    integrationReport = await invoke<IntegrationReport>(command);
  } catch (error) {
    integrationError = String(error);
  }
  integrationBusy = false;
  renderIntegrations();
  render();
}

installHooks.addEventListener("click", () => void updateIntegrations("install_integrations"));
removeHooks.addEventListener("click", () => void updateIntegrations("uninstall_integrations"));

function render(): void {
  board.textContent = game.render();
  status.textContent = statusText();
  score.textContent = game.score.toString().padStart(5, "0");
  jobs.textContent = activeTurns.size.toString().padStart(2, "0");
  agentLabel.textContent = agent;
  message.textContent = messageText();
  message.classList.toggle("visible", game.phase !== "running");
}

function keyDirection(key: string): Direction | undefined {
  const directions: Record<string, Direction> = {
    ArrowUp: "up",
    w: "up",
    ArrowDown: "down",
    s: "down",
    ArrowLeft: "left",
    a: "left",
    ArrowRight: "right",
    d: "right",
  };
  return directions[key];
}

window.addEventListener("keydown", (event) => {
  const direction = keyDirection(event.key);
  if (direction) {
    event.preventDefault();
    game.queue(direction);
  } else if (event.key === " ") {
    event.preventDefault();
    game.togglePause();
  } else if (event.key.toLowerCase() === "r") {
    game.reset();
  }
  render();
});

await listen<AgentEvent>("agent-event", ({ payload }) => {
  const key = `${payload.source}:${payload.session_id}:${payload.turn_id}`;
  if (payload.action === "start") {
    activeTurns.add(key);
    agent = payload.source === "claude" ? "CLAUDE-CODE" : "CODEX";
    game.reset();
  } else if (payload.action === "attention") {
    game.phase = "attention";
  } else {
    activeTurns.delete(key);
    if (activeTurns.size === 0) game.phase = "finished";
  }
  render();
});

function tick(): void {
  game.advance();
  render();
  window.setTimeout(tick, Math.max(70, 135 - game.score * 3));
}

render();
renderIntegrations();
void refreshIntegrations();
tick();
