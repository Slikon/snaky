use serde::Serialize;
use serde_json::{json, Map, Value};
use std::{
    fs,
    path::{Path, PathBuf},
};

const MARKER: &str = "AGENT_SNAKE_HOOK=1 ";

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IntegrationState {
    name: &'static str,
    config_path: String,
    detected: bool,
    installed: bool,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IntegrationReport {
    codex: IntegrationState,
    claude: IntegrationState,
    executable_path: String,
}

struct Integration {
    name: &'static str,
    source: &'static str,
    config_path: PathBuf,
    events: &'static [(&'static str, &'static str, u64)],
}

const CODEX_EVENTS: &[(&str, &str, u64)] = &[
    ("UserPromptSubmit", "start", 3),
    ("PermissionRequest", "attention", 1),
    ("Interrupt", "interrupt", 1),
    ("Stop", "stop", 1),
];

const CLAUDE_EVENTS: &[(&str, &str, u64)] = &[
    ("UserPromptSubmit", "start", 3),
    ("PermissionRequest", "attention", 1),
    ("Stop", "stop", 1),
];

fn home_dir() -> Result<PathBuf, String> {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .ok_or_else(|| "Could not find the current user's home directory".into())
}

fn integrations() -> Result<[Integration; 2], String> {
    let home = home_dir()?;
    Ok([
        Integration {
            name: "Codex",
            source: "codex",
            config_path: home.join(".codex/hooks.json"),
            events: CODEX_EVENTS,
        },
        Integration {
            name: "Claude Code",
            source: "claude",
            config_path: home.join(".claude/settings.json"),
            events: CLAUDE_EVENTS,
        },
    ])
}

fn executable_path() -> Result<PathBuf, String> {
    std::env::current_exe().map_err(|error| format!("Could not locate Agent Snake: {error}"))
}

fn shell_quote(path: &Path) -> String {
    format!("'{}'", path.to_string_lossy().replace('\'', "'\"'\"'"))
}

fn hook_command(executable: &Path, source: &str, action: &str) -> String {
    format!(
        "{MARKER}{} hook {source} {action}",
        shell_quote(executable)
    )
}

fn hook_group(executable: &Path, source: &str, action: &str, timeout: u64) -> Value {
    json!({
        "hooks": [{
            "type": "command",
            "command": hook_command(executable, source, action),
            "timeout": timeout
        }]
    })
}

fn read_config(path: &Path) -> Result<Value, String> {
    if !path.exists() {
        return Ok(json!({}));
    }
    let text = fs::read_to_string(path)
        .map_err(|error| format!("Could not read {}: {error}", path.display()))?;
    serde_json::from_str(&text)
        .map_err(|error| format!("{} is not valid JSON: {error}", path.display()))
}

fn hooks_object_mut(config: &mut Value) -> Result<&mut Map<String, Value>, String> {
    let root = config
        .as_object_mut()
        .ok_or_else(|| "The settings file must contain a JSON object".to_string())?;
    if !root.contains_key("hooks") {
        root.insert("hooks".into(), json!({}));
    }
    root.get_mut("hooks")
        .and_then(Value::as_object_mut)
        .ok_or_else(|| "The existing `hooks` setting must be a JSON object".to_string())
}

fn is_managed_group(group: &Value) -> bool {
    group
        .get("hooks")
        .and_then(Value::as_array)
        .is_some_and(|hooks| {
            hooks.iter().any(|hook| {
                hook.get("command")
                    .and_then(Value::as_str)
                    .is_some_and(|command| command.starts_with(MARKER))
            })
        })
}

fn remove_managed_hooks(config: &mut Value) -> Result<(), String> {
    let Some(root) = config.as_object_mut() else {
        return Err("The settings file must contain a JSON object".into());
    };
    let Some(hooks_value) = root.get_mut("hooks") else {
        return Ok(());
    };
    let hooks = hooks_value
        .as_object_mut()
        .ok_or_else(|| "The existing `hooks` setting must be a JSON object".to_string())?;
    hooks.retain(|_, groups| {
        let Some(groups) = groups.as_array_mut() else {
            return true;
        };
        groups.retain(|group| !is_managed_group(group));
        !groups.is_empty()
    });
    Ok(())
}

fn add_hooks(
    config: &mut Value,
    integration: &Integration,
    executable: &Path,
) -> Result<(), String> {
    remove_managed_hooks(config)?;
    let hooks = hooks_object_mut(config)?;
    for (event, action, timeout) in integration.events {
        let groups = hooks.entry(*event).or_insert_with(|| json!([]));
        let groups = groups
            .as_array_mut()
            .ok_or_else(|| format!("The existing `{event}` hooks setting must be an array"))?;
        groups.push(hook_group(executable, integration.source, action, *timeout));
    }
    Ok(())
}

fn write_config(path: &Path, config: &Value) -> Result<(), String> {
    let parent = path
        .parent()
        .ok_or_else(|| format!("Invalid settings path: {}", path.display()))?;
    fs::create_dir_all(parent)
        .map_err(|error| format!("Could not create {}: {error}", parent.display()))?;

    if path.exists() {
        let backup = PathBuf::from(format!("{}.agent-snake.backup", path.display()));
        if !backup.exists() {
            fs::copy(path, &backup)
                .map_err(|error| format!("Could not back up {}: {error}", path.display()))?;
        }
    }

    let temporary = PathBuf::from(format!("{}.agent-snake.tmp", path.display()));
    let mut output = serde_json::to_string_pretty(config)
        .map_err(|error| format!("Could not serialize {}: {error}", path.display()))?;
    output.push('\n');
    fs::write(&temporary, output)
        .map_err(|error| format!("Could not write {}: {error}", temporary.display()))?;
    fs::rename(&temporary, path)
        .map_err(|error| format!("Could not replace {}: {error}", path.display()))
}

fn is_installed(integration: &Integration, executable: &Path) -> Result<bool, String> {
    if !integration.config_path.exists() {
        return Ok(false);
    }
    let config = read_config(&integration.config_path)?;
    let Some(hooks) = config.get("hooks").and_then(Value::as_object) else {
        return Ok(false);
    };
    Ok(integration.events.iter().all(|(event, action, _)| {
        let expected = hook_command(executable, integration.source, action);
        hooks
            .get(*event)
            .and_then(Value::as_array)
            .is_some_and(|groups| {
                groups.iter().any(|group| {
                    group
                        .get("hooks")
                        .and_then(Value::as_array)
                        .is_some_and(|commands| {
                            commands.iter().any(|command| {
                                command.get("command").and_then(Value::as_str)
                                    == Some(expected.as_str())
                            })
                        })
                })
            })
    }))
}

fn report() -> Result<IntegrationReport, String> {
    let executable = executable_path()?;
    let [codex, claude] = integrations()?;
    let state = |integration: &Integration| -> Result<IntegrationState, String> {
        Ok(IntegrationState {
            name: integration.name,
            config_path: integration.config_path.display().to_string(),
            detected: integration
                .config_path
                .parent()
                .is_some_and(Path::exists),
            installed: is_installed(integration, &executable)?,
        })
    };
    Ok(IntegrationReport {
        codex: state(&codex)?,
        claude: state(&claude)?,
        executable_path: executable.display().to_string(),
    })
}

#[tauri::command]
pub fn integration_status() -> Result<IntegrationReport, String> {
    report()
}

#[tauri::command]
pub fn install_integrations() -> Result<IntegrationReport, String> {
    let executable = executable_path()?;
    for integration in integrations()? {
        let mut config = read_config(&integration.config_path)?;
        add_hooks(&mut config, &integration, &executable)?;
        write_config(&integration.config_path, &config)?;
    }
    report()
}

#[tauri::command]
pub fn uninstall_integrations() -> Result<IntegrationReport, String> {
    for integration in integrations()? {
        if !integration.config_path.exists() {
            continue;
        }
        let mut config = read_config(&integration.config_path)?;
        remove_managed_hooks(&mut config)?;
        write_config(&integration.config_path, &config)?;
    }
    report()
}
