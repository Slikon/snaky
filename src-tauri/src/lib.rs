use serde::{Deserialize, Serialize};
use std::{collections::HashSet, sync::Arc};
use tauri::{Emitter, Manager};
use tokio::{
    io::{AsyncBufReadExt, BufReader},
    net::TcpListener,
    sync::Mutex,
    time::{sleep, Duration},
};

const EVENT_ADDRESS: &str = "127.0.0.1:49271";

#[derive(Clone, Debug, Deserialize, Serialize)]
struct AgentEvent {
    source: String,
    action: String,
    session_id: String,
    turn_id: String,
}

impl AgentEvent {
    fn key(&self) -> String {
        format!("{}:{}:{}", self.source, self.session_id, self.turn_id)
    }
}

async fn run_event_server(app: tauri::AppHandle) {
    let Ok(listener) = TcpListener::bind(EVENT_ADDRESS).await else {
        return;
    };
    let active_turns = Arc::new(Mutex::new(HashSet::<String>::new()));

    loop {
        let Ok((stream, _)) = listener.accept().await else {
            continue;
        };
        let app = app.clone();
        let active_turns = active_turns.clone();
        tauri::async_runtime::spawn(async move {
            let mut line = String::new();
            let mut reader = BufReader::new(stream);
            if reader.read_line(&mut line).await.is_err() {
                return;
            }
            let Ok(event) = serde_json::from_str::<AgentEvent>(&line) else {
                return;
            };

            let should_minimize = {
                let mut turns = active_turns.lock().await;
                match event.action.as_str() {
                    "start" => {
                        turns.insert(event.key());
                        false
                    }
                    "stop" | "interrupt" => {
                        turns.remove(&event.key());
                        turns.is_empty()
                    }
                    "attention" => true,
                    _ => false,
                }
            };

            let _ = app.emit("agent-event", &event);
            let Some(window) = app.get_webview_window("main") else {
                return;
            };
            if event.action == "start" {
                let _ = window.unminimize();
                let _ = window.show();
                let _ = window.set_focus();
            } else if should_minimize {
                sleep(Duration::from_millis(350)).await;
                let _ = window.minimize();
            }
        });
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _, _| {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.unminimize();
                let _ = window.show();
                let _ = window.set_focus();
            }
        }))
        .setup(|app| {
            let handle = app.handle().clone();
            tauri::async_runtime::spawn(run_event_server(handle));
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running Agent Snake");
}
