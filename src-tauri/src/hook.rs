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
    let session_id = string_field(&hook, "session_id").unwrap_or("unknown-session");
    let turn_id = string_field(&hook, "turn_id")
        .or_else(|| string_field(&hook, "agent_id"))
        .unwrap_or(session_id);
    let payload = json!({
        "source": source,
        "action": action,
        "session_id": session_id,
        "turn_id": turn_id,
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
    Command::new(executable)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .is_ok()
}
