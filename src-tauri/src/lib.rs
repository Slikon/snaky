use serde::{Deserialize, Serialize};
use std::{collections::HashMap, sync::Mutex};
use tauri::{Emitter, Manager};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, ShortcutState};
use tokio::{io::{AsyncBufReadExt, AsyncReadExt, BufReader}, net::TcpListener, time::{timeout, Duration}};

mod hook;
mod integrations;
mod observer;
pub use hook::run as run_hook;
pub use integrations::install_integrations;

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct AgentEvent {
    pub source: String,
    pub action: String,
    pub session_id: String,
    #[serde(default)] pub turn_id: String,
    #[serde(default)] pub codex_binary: Option<String>,
}
impl AgentEvent {
    pub fn key(&self) -> String { format!("{}:{}", self.source, self.session_id) }
}
#[derive(Default)]
pub struct SessionState {
    pub turns: HashMap<String, AgentEvent>,
    attention: HashMap<String, bool>,
    game_visible: bool,
    dismissed: bool,
    shortcut_available: bool,
    main_ready: bool,
}
#[derive(Default)]
pub struct Runtime(pub Mutex<SessionState>);
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct Snapshot {
    working: usize,
    attention: bool,
    source: String,
    game_visible: bool,
    shortcut_available: bool,
}
impl SessionState {
    fn snapshot(&self) -> Snapshot {
        Snapshot { working: self.turns.len(), attention: self.attention.values().any(|v| *v),
            source: self.turns.values().map(|e| e.source.as_str()).min().unwrap_or("agent").into(),
            game_visible: self.game_visible, shortcut_available: self.shortcut_available }
    }
    fn is_current(&self, event: &AgentEvent) -> bool {
        self.turns.get(&event.key()).is_some_and(|e| event.turn_id.is_empty() || e.turn_id.is_empty() || e.turn_id == event.turn_id)
    }
    fn apply(&mut self, event: &AgentEvent) -> bool {
        match event.action.as_str() {
            "start" => {
                if self.turns.get(&event.key()).is_some_and(|e| e.turn_id == event.turn_id) { return false; }
                self.turns.insert(event.key(), event.clone());
                self.attention.retain(|key, _| !key.starts_with(&format!("{}:", event.key())));
                self.dismissed = false;
            }
            "attention" | "approval_wait" if self.is_current(event) => {
                let kind = if event.action == "approval_wait" { "approval" } else { "question" };
                self.attention.insert(format!("{}:{kind}", event.key()), true);
                self.game_visible = false;
            }
            "resume" | "approval_resume" if self.is_current(event) => {
                let kind = if event.action == "approval_resume" { "approval" } else { "question" };
                self.attention.remove(&format!("{}:{kind}", event.key()));
            }
            "stop" | "interrupt" if self.is_current(event) => {
                self.turns.remove(&event.key()); self.attention.retain(|key, _| !key.starts_with(&format!("{}:", event.key())));
                if self.turns.is_empty() { self.game_visible = false; }
            }
            _ => return false,
        }
        true
    }
}
fn sync_windows(app: &tauri::AppHandle, state: &SessionState) {
    let snapshot = state.snapshot();
    let _ = app.emit("companion-state", &snapshot);
    if let Some(main) = app.get_webview_window("main") {
        if state.game_visible && state.main_ready { if !main.is_visible().unwrap_or(false) { let _ = main.show(); } }
        else { let _ = main.hide(); }
    }
    if let Some(launcher) = app.get_webview_window("launcher") {
        if !state.turns.is_empty() && !state.game_visible && !state.dismissed {
            if let Ok(Some(monitor)) = launcher.current_monitor().or_else(|_| launcher.primary_monitor()) {
                let area = monitor.work_area();
                let scale = monitor.scale_factor();
                let x = area.position.x + area.size.width as i32 - (132.0 * scale) as i32;
                let y = area.position.y + (20.0 * scale) as i32;
                let _ = launcher.set_position(tauri::PhysicalPosition::new(x, y));
            }
            if !launcher.is_visible().unwrap_or(false) { let _ = launcher.show(); } // Launcher cannot become key.
        } else { let _ = launcher.hide(); }
    }
}
fn dispatch(app: &tauri::AppHandle, event: AgentEvent) {
    let handle = app.clone();
    let _ = app.run_on_main_thread(move || {
        let runtime = handle.state::<Runtime>();
        let Ok(mut state) = runtime.0.lock() else { return; };
        let was_focused = handle.get_webview_window("main").is_some_and(|w| w.is_focused().unwrap_or(false));
        if !state.apply(&event) { return; }
        sync_windows(&handle, &state);
        if was_focused && !state.game_visible { yield_focus(&handle); }
        if event.source == "codex" && event.action == "start" && event.codex_binary.is_some() {
            tauri::async_runtime::spawn(observer::watch(handle.clone(), event));
        }
    });
}
pub fn confirmed_attention(app: &tauri::AppHandle, event: &AgentEvent, waiting: bool) {
    let mut event = event.clone();
    event.action = if waiting { "approval_wait" } else { "approval_resume" }.into();
    dispatch(app, event);
}
// Hiding the macOS application yields back to the previous app. Showing it
// again restores only the non-focusable companion, without activating Snake.
fn yield_focus(_app: &tauri::AppHandle) {
    #[cfg(target_os = "macos")]
    { let _ = _app.hide(); let _ = _app.show(); }
}
#[tauri::command]
fn companion_state(app: tauri::AppHandle) -> Snapshot { app.state::<Runtime>().0.lock().unwrap().snapshot() }
#[tauri::command]
fn ready(app: tauri::AppHandle, window: tauri::WebviewWindow) {
    let runtime = app.state::<Runtime>();
    let mut state = runtime.0.lock().unwrap();
    if window.label() == "main" { state.main_ready = true; }
    sync_windows(&app, &state);
}
#[tauri::command]
fn open_game(app: tauri::AppHandle) {
    let runtime = app.state::<Runtime>();
    let mut state = runtime.0.lock().unwrap();
    state.game_visible = true;
    sync_windows(&app, &state);
    if let Some(window) = app.get_webview_window("main") { let _ = window.set_focus(); }
}
#[tauri::command]
fn tuck_game(app: tauri::AppHandle) {
    let runtime = app.state::<Runtime>();
    let mut state = runtime.0.lock().unwrap();
    state.game_visible = false;
    sync_windows(&app, &state);
    yield_focus(&app);
}
#[tauri::command]
fn dismiss_companion(app: tauri::AppHandle) {
    let runtime = app.state::<Runtime>();
    let mut state = runtime.0.lock().unwrap();
    state.dismissed = true;
    sync_windows(&app, &state);
}
async fn run_event_server(app: tauri::AppHandle) {
    let Ok(listener) = TcpListener::bind("127.0.0.1:49271").await else { return; };
    loop {
        let Ok((stream, _)) = listener.accept().await else { continue; };
        let handle = app.clone();
        tauri::async_runtime::spawn(async move {
            let mut line = String::new();
            let mut reader = BufReader::new(stream.take(16_384));
            if !matches!(timeout(Duration::from_secs(1), reader.read_line(&mut line)).await, Ok(Ok(_))) { return; }
            if let Ok(value) = serde_json::from_str::<serde_json::Value>(&line) {
                // Ignore old adapters that conflated permission review with human attention.
                if value["source"] == "codex" && value["action"] == "attention" && value["attention_kind"] != "blocking_question" { return; }
                if let Ok(event) = serde_json::from_value::<AgentEvent>(value) {
                    if matches!(event.action.as_str(), "start" | "stop" | "interrupt" | "attention" | "resume") { dispatch(&handle, event); }
                }
            }
        });
    }
}
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .manage(Runtime::default())
        .plugin(tauri_plugin_single_instance::init(|app, args, _| {
            if !args.iter().any(|a| a == "--background") { open_game(app.clone()); }
        }))
        .plugin(tauri_plugin_global_shortcut::Builder::new().with_handler(|app, _, event| {
            if event.state == ShortcutState::Pressed {
                if app.state::<Runtime>().0.lock().unwrap().game_visible { tuck_game(app.clone()); }
                else { open_game(app.clone()); }
            }
        }).build())
        .setup(|app| {
            #[cfg(target_os = "macos")]
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);
            let shortcut = app.global_shortcut().register("Alt+Shift+S").is_ok();
            { let runtime = app.state::<Runtime>(); let mut state = runtime.0.lock().unwrap();
              state.shortcut_available = shortcut;
              state.game_visible = !std::env::args().any(|arg| arg == "--background"); }
            tauri::async_runtime::spawn(run_event_server(app.handle().clone()));
            Ok(())
        })
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                if window.label() == "main" { tuck_game(window.app_handle().clone()); }
                else { dismiss_companion(window.app_handle().clone()); }
            }
        })
        .invoke_handler(tauri::generate_handler![companion_state, ready, open_game, tuck_game, dismiss_companion,
            integrations::integration_status, integrations::install_integrations, integrations::uninstall_integrations])
        .run(tauri::generate_context!()).expect("error while running Snaky");
}
#[cfg(test)]
mod tests {
    use super::*;
    fn event(action: &str, session: &str, turn: &str) -> AgentEvent {
        AgentEvent { source:"codex".into(),action:action.into(),session_id:session.into(),turn_id:turn.into(),codex_binary:None }
    }
    #[test] fn start_does_not_open_game_and_duplicate_does_not_reset() {
        let mut s=SessionState::default(); assert!(s.apply(&event("start","a","1"))); assert!(!s.game_visible);
        s.dismissed=true; assert!(!s.apply(&event("start","a","1"))); assert!(s.dismissed);
    }
    #[test] fn stale_stop_cannot_close_new_turn() {
        let mut s=SessionState::default();s.apply(&event("start","a","1"));s.apply(&event("start","a","2"));
        assert!(!s.apply(&event("stop","a","1")));assert_eq!(s.turns.len(),1);
    }
    #[test] fn attention_resumes_without_reopening() {
        let mut s=SessionState::default();s.apply(&event("start","a","1"));s.game_visible=true;
        s.apply(&event("attention","a","1"));assert!(!s.game_visible);assert!(s.snapshot().attention);
        s.apply(&event("resume","a","1"));assert!(!s.snapshot().attention);assert!(!s.game_visible);
    }
    #[test] fn approval_observer_does_not_clear_a_blocking_question() {
        let mut s=SessionState::default();s.apply(&event("start","a","1"));s.apply(&event("attention","a","1"));
        s.apply(&event("approval_resume","a","1"));assert!(s.snapshot().attention);
        s.apply(&event("resume","a","1"));assert!(!s.snapshot().attention);
    }
    #[test] fn other_sessions_and_unknown_events_do_not_close_game() {
        let mut s=SessionState::default();s.apply(&event("start","a","1"));s.apply(&event("start","b","1"));s.game_visible=true;
        s.apply(&event("stop","a","1"));assert!(s.game_visible);
        assert!(!s.apply(&event("attention","unknown","1"))); assert!(s.game_visible);
        s.apply(&event("stop","b","1"));assert!(!s.game_visible);
    }
}
