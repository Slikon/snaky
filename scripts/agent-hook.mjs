#!/usr/bin/env node

import { spawn } from "node:child_process";
import net from "node:net";
import os from "node:os";
import path from "node:path";
import process from "node:process";
import { fileURLToPath } from "node:url";

const HOST = "127.0.0.1";
const PORT = 49271;
const [, , source, action] = process.argv;

let input = "";
for await (const chunk of process.stdin) input += chunk;
let hook = {};
try { hook = JSON.parse(input); } catch { /* lifecycle bridges must never block the agent */ }

const session = String(hook.session_id ?? "unknown-session");
const turn = String(hook.turn_id ?? hook.session_id ?? "unknown-turn");
const payload = `${JSON.stringify({ source, action, session_id: session, turn_id: turn })}\n`;

function send() {
  return new Promise((resolve) => {
    const socket = net.createConnection({ host: HOST, port: PORT });
    socket.setTimeout(180);
    socket.on("connect", () => socket.end(payload, () => resolve(true)));
    socket.on("timeout", () => { socket.destroy(); resolve(false); });
    socket.on("error", () => resolve(false));
  });
}

function candidates() {
  const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
  const extension = process.platform === "win32" ? ".exe" : "";
  return [
    process.env.AGENT_SNAKE_BIN,
    path.join(root, "src-tauri", "target", "debug", `agent-snake${extension}`),
    path.join(root, "src-tauri", "target", "release", `agent-snake${extension}`),
    process.platform === "darwin" ? "/Applications/Agent Snake.app/Contents/MacOS/agent-snake" : undefined,
    process.platform === "win32" ? path.join(process.env.LOCALAPPDATA ?? "", "Agent Snake", "agent-snake.exe") : undefined,
    process.platform === "linux" ? path.join(os.homedir(), ".local", "bin", "agent-snake") : undefined,
  ].filter(Boolean);
}

async function launch() {
  const fs = await import("node:fs");
  const executable = candidates().find((candidate) => fs.existsSync(candidate));
  if (!executable) return false;
  const child = spawn(executable, [], { detached: true, stdio: "ignore" });
  child.unref();
  return true;
}

if (!(await send()) && action === "start" && (await launch())) {
  for (let attempt = 0; attempt < 40; attempt += 1) {
    await new Promise((resolve) => setTimeout(resolve, 50));
    if (await send()) break;
  }
}

if (action === "stop") process.stdout.write('{"continue":true}\n');
