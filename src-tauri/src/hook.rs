use serde_json::{json, Value};
use std::{
    io::{Read, Write},
    net::{SocketAddr, TcpStream},
    process::{Command, Stdio},
    thread,
    time::Duration,
};

const EVENT_PORT: u16 = 49271;
const CONNECT_TIMEOUT: Duration = Duration::from_millis(180);

pub fn run(source: &str, action: &str) -> i32 {
    let mut input = String::new();
    let _ = std::io::stdin().read_to_string(&mut input);
    let hook: Value = serde_json::from_str(&input).unwrap_or_default();
    let Some(action) = classify_action(source, action, &hook) else { return 0; };
    let session_id = string_field(&hook, "session_id").unwrap_or("unknown-session");
    let turn_id = string_field(&hook, "turn_id")
        .or_else(|| string_field(&hook, "agent_id"))
        .unwrap_or(session_id);
    let payload = json!({
        "source": source,
        "action": action,
        "attention_kind": if action == "attention" { Some("blocking_question") } else { None },
        "session_id": session_id,
        "turn_id": turn_id,
        "codex_binary": if source == "codex" { find_codex() } else { None },
    });

    if !send(&payload) && action == "start" && launch_app() {
        for _ in 0..40 {
            thread::sleep(Duration::from_millis(50));
            if send(&payload) {
                break;
            }
        }
    }

    // Lifecycle hooks must never block the coding agent.
    if action == "stop" {
        println!("{}", r#"{"continue":true}"#);
    }
    0
}

fn string_field<'a>(value: &'a Value, field: &str) -> Option<&'a str> {
    value.get(field).and_then(Value::as_str)
}

fn send(payload: &Value) -> bool {
    let address = SocketAddr::from(([127, 0, 0, 1], EVENT_PORT));
    let Ok(mut stream) = TcpStream::connect_timeout(&address, CONNECT_TIMEOUT) else {
        return false;
    };
    let _ = stream.set_write_timeout(Some(CONNECT_TIMEOUT));
    writeln!(stream, "{payload}").is_ok()
}

fn launch_app() -> bool {
    let Ok(executable) = std::env::current_exe() else {
        return false;
    };
    // LaunchServices owns the packaged app's lifetime, independently of the
    // short-lived hook process (and any process-group cleanup by its caller).
    #[cfg(target_os = "macos")]
    if let Some(bundle) = executable.ancestors().nth(3).filter(|path| path.extension().is_some_and(|ext| ext == "app")) {
        return Command::new("/usr/bin/open").args(["-g", "-j", "-a"]).arg(bundle)
            .args(["--args", "--background"])
            .stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null())
            .status().is_ok_and(|status| status.success());
    }
    Command::new(executable)
        .arg("--background")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .is_ok()
}

// PermissionRequest is a pre-decision hook, including automatic review.
// Never infer a human prompt from it, even from an older installed hook.
fn classify_action<'a>(source: &str, action: &'a str, hook: &Value) -> Option<&'a str> {
    if hook.get("agent_id").is_some() || hook.get("subagent").is_some_and(|v| !v.is_null()) { return None; }
    match action {
        "attention" if source == "codex" => None,
        "attention" | "notification" if source == "claude" =>
            (string_field(hook, "hook_event_name") == Some("Notification") &&
             string_field(hook, "notification_type") == Some("permission_prompt")).then_some("attention"),
        "question" => {
            let tool = string_field(hook, "tool_name").unwrap_or("");
            let input = &hook["tool_input"];
            let blocking = input.get("is_blocking").and_then(Value::as_bool) != Some(false);
            let automatic = input.get("auto_resolution_ms").is_some_and(|v| !v.is_null());
            (blocking && !automatic && matches!(tool, "request_user_input" | "AskUserQuestion")).then_some("attention")
        }
        "start" | "stop" | "interrupt" | "resume" => Some(action),
        _ => None,
    }
}
fn find_codex() -> Option<String> {
    std::env::var_os("PATH").and_then(|path| std::env::split_paths(&path)
        .map(|dir| dir.join(if cfg!(windows) { "codex.cmd" } else { "codex" }))
        .find(|path| path.is_file()).map(|path| path.to_string_lossy().into_owned()))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn automatic_permission_review_is_not_attention() {
        assert_eq!(classify_action("codex", "attention", &json!({"hook_event_name":"PermissionRequest","permission_mode":"default"})), None);
        assert_eq!(classify_action("claude", "attention", &json!({"hook_event_name":"PermissionRequest"})), None);
    }
    #[test] fn only_real_claude_permission_notifications_interrupt() {
        assert_eq!(classify_action("claude", "notification", &json!({"hook_event_name":"Notification","notification_type":"permission_prompt"})), Some("attention"));
        assert_eq!(classify_action("claude", "notification", &json!({"hook_event_name":"Notification","notification_type":"idle_prompt"})), None);
    }
    #[test] fn questions_must_block_and_require_a_person() {
        assert_eq!(classify_action("codex", "question", &json!({"tool_name":"request_user_input"})), Some("attention"));
        for input in [json!({"is_blocking":false}),json!({"auto_resolution_ms":1000})] {
            assert_eq!(classify_action("codex", "question", &json!({"tool_name":"request_user_input","tool_input":input})), None);
        }
        assert_eq!(classify_action("codex", "question", &json!({"tool_name":"exec_command"})), None);
    }
    #[test] fn child_hooks_do_not_interrupt_parent_game() {
        assert_eq!(classify_action("codex", "stop", &json!({"agent_id":"child"})), None);
    }
}
