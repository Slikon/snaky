//! Observe the existing Codex daemon without attaching to or controlling a turn.
//!
//! PermissionRequest hooks run before automatic approval review. Only the live
//! `waitingOnApproval` flag confirms that control has actually reached the user.
use std::{io, process::Stdio, time::Duration};

use serde_json::{json, Value};
use tauri::Manager;
use tokio::{
    io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader},
    process::{Child, ChildStdin, ChildStdout, Command},
    time::{sleep, timeout},
};

const IO_TIMEOUT: Duration = Duration::from_secs(2);
const POLL_INTERVAL: Duration = Duration::from_millis(800);
const RETRY_INTERVAL: Duration = Duration::from_secs(5);
const MAX_MESSAGE_BYTES: u64 = 512 * 1024;

struct Observer {
    // Dropping an observer terminates its proxy, including after an I/O timeout.
    _child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
    next_id: u64,
}

impl Observer {
    async fn connect(binary: &str) -> io::Result<Self> {
        // `proxy` connects to an already-running daemon. Never substitute plain
        // `app-server`, `daemon start`, `thread/resume`, or `thread/attach` here.
        let mut child = Command::new(binary)
            .args(["app-server", "proxy"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .kill_on_drop(true)
            .spawn()?;
        let stdin = child.stdin.take().ok_or_else(|| unavailable("missing proxy stdin"))?;
        let stdout = child.stdout.take().ok_or_else(|| unavailable("missing proxy stdout"))?;
        let mut observer = Self {
            _child: child,
            stdin,
            stdout: BufReader::new(stdout),
            next_id: 1,
        };
        observer.request("initialize", json!({
            "clientInfo": { "name": "agent_snake_observer", "version": env!("CARGO_PKG_VERSION") },
            "capabilities": { "experimentalApi": true }
        })).await?;
        timeout(IO_TIMEOUT, observer.write(json!({ "method": "initialized" })))
            .await
            .map_err(|_| unavailable("proxy initialization timed out"))??;
        Ok(observer)
    }

    async fn write(&mut self, message: Value) -> io::Result<()> {
        let mut bytes = serde_json::to_vec(&message)?;
        bytes.push(b'\n');
        self.stdin.write_all(&bytes).await?;
        self.stdin.flush().await
    }

    async fn request(&mut self, method: &str, params: Value) -> io::Result<Value> {
        let id = self.next_id;
        self.next_id += 1;
        timeout(IO_TIMEOUT, async {
            self.write(json!({ "id": id, "method": method, "params": params })).await?;
            loop {
                let mut line = String::new();
                // A malformed or oversized peer message cannot grow memory without limit.
                let bytes = (&mut self.stdout).take(MAX_MESSAGE_BYTES).read_line(&mut line).await?;
                if bytes == 0 || !line.ends_with('\n') {
                    return Err(unavailable("proxy stream ended or message too large"));
                }
                let message: Value = serde_json::from_str(&line)?;
                // Ignore notifications and server requests; we never answer approvals.
                if message.get("method").is_some() || message.get("id") != Some(&json!(id)) {
                    continue;
                }
                if message.get("error").is_some() {
                    return Err(unavailable("proxy request failed"));
                }
                return message.get("result").cloned().ok_or_else(|| unavailable("missing proxy result"));
            }
        }).await.map_err(|_| unavailable("proxy request timed out"))?
    }
}

fn unavailable(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::ConnectionAborted, message)
}

/// Unknown, unloaded, and malformed states are not evidence of user attention.
fn waiting_on_approval(result: &Value, session_id: &str) -> Option<bool> {
    let thread = result.get("thread")?;
    if thread.get("id")?.as_str()? != session_id {
        return None;
    }
    let status = thread.get("status")?;
    match status.get("type")?.as_str()? {
        "active" => {
            let flags = status.get("activeFlags")?.as_array()?;
            // Malformed flags must not accidentally clear a previously confirmed wait.
            if flags.iter().any(|flag| !flag.is_string()) {
                return None;
            }
            Some(flags.iter().any(|flag| flag == "waitingOnApproval"))
        }
        "idle" => Some(false),
        _ => None,
    }
}

fn is_current(app: &tauri::AppHandle, event: &crate::AgentEvent) -> bool {
    let runtime = app.state::<crate::Runtime>();
    let Ok(state) = runtime.0.lock() else { return false; };
    state.turns.get(&format!("{}:{}", event.source, event.session_id))
        .is_some_and(|current| current.turn_id == event.turn_id)
}

pub async fn watch(app: tauri::AppHandle, event: crate::AgentEvent) {
    let Some(binary) = event.codex_binary.as_deref().filter(|binary| !binary.is_empty()) else {
        // Older clients without a captured binary keep the conservative hook behavior.
        return;
    };
    let mut connection: Option<Observer> = None;
    let mut last_confirmed = None;
    while is_current(&app, &event) {
        if connection.is_none() {
            match Observer::connect(binary).await {
                Ok(observer) => connection = Some(observer),
                Err(_) => {
                    sleep(RETRY_INTERVAL).await;
                    continue;
                }
            }
        }
        if !is_current(&app, &event) { break; }
        let result = connection.as_mut().expect("connected observer")
            .request("thread/read", json!({ "threadId": event.session_id, "includeTurns": false }))
            .await;
        if !is_current(&app, &event) { break; }
        match result {
            Ok(result) => {
                if let Some(waiting) = waiting_on_approval(&result, &event.session_id) {
                    if last_confirmed != Some(waiting) {
                        crate::confirmed_attention(&app, &event, waiting);
                        last_confirmed = Some(waiting);
                    }
                }
                sleep(POLL_INTERVAL).await;
            }
            Err(_) => {
                connection = None;
                // Failure to observe is never a reason to interrupt the game.
                sleep(RETRY_INTERVAL).await;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn status(value: Value) -> Value {
        json!({ "thread": { "id": "session", "status": value } })
    }

    #[test]
    fn only_confirmed_approval_wait_requests_attention() {
        assert_eq!(waiting_on_approval(&status(json!({ "type": "active", "activeFlags": ["waitingOnApproval"] })), "session"), Some(true));
        assert_eq!(waiting_on_approval(&status(json!({ "type": "active", "activeFlags": ["waitingOnUserInput", "waitingOnApproval"] })), "session"), Some(true));
        assert_eq!(waiting_on_approval(&status(json!({ "type": "active", "activeFlags": ["waitingOnUserInput"] })), "session"), Some(false));
        assert_eq!(waiting_on_approval(&status(json!({ "type": "active", "activeFlags": [] })), "session"), Some(false));
        assert_eq!(waiting_on_approval(&status(json!({ "type": "idle" })), "session"), Some(false));
    }

    #[test]
    fn hooks_errors_and_unknown_status_are_not_confirmation() {
        for value in [
            json!({ "hook_event_name": "PermissionRequest" }),
            json!({ "error": { "message": "daemon unavailable" } }),
            json!(null),
            status(json!({ "type": "notLoaded" })),
            status(json!({ "type": "systemError" })),
            status(json!({ "type": "reviewingApprovalRequests" })),
            status(json!({ "type": "active" })),
            status(json!({ "type": "active", "activeFlags": "waitingOnApproval" })),
            status(json!({ "type": "active", "activeFlags": [null] })),
        ] {
            assert_eq!(waiting_on_approval(&value, "session"), None, "{value}");
        }
    }

    #[test]
    fn approval_from_another_thread_is_ignored() {
        let value = status(json!({ "type": "active", "activeFlags": ["waitingOnApproval"] }));
        assert_eq!(waiting_on_approval(&value, "another-session"), None);
    }
}
