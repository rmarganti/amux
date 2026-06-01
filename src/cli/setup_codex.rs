use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

use serde_json::{Map, Value, json};

use crate::error::AmuxError;

const HOOK_SCRIPT: &str = include_str!("../../plugin/codex/amux-status.sh");
const MANAGED_EVENTS: &[&str] = &[
    "SessionStart",
    "UserPromptSubmit",
    "PreToolUse",
    "PostToolUse",
    "PermissionRequest",
    "SubagentStart",
    "SubagentStop",
    "Stop",
];

fn event_requires_matcher(event_name: &str) -> bool {
    matches!(
        event_name,
        "PreToolUse" | "PostToolUse" | "PermissionRequest" | "SubagentStart" | "SubagentStop"
    )
}

fn version_from_content(content: &str) -> Option<&str> {
    content.lines().nth(1)?.strip_prefix("# amux-status ")
}

fn codex_dir() -> Result<PathBuf, AmuxError> {
    let home = dirs::home_dir()
        .ok_or_else(|| AmuxError::Setup("could not determine home directory".to_string()))?;
    Ok(home.join(".codex"))
}

fn hook_script_path() -> Result<PathBuf, AmuxError> {
    Ok(codex_dir()?.join("hooks").join("amux-status.sh"))
}

fn hooks_json_path() -> Result<PathBuf, AmuxError> {
    Ok(codex_dir()?.join("hooks.json"))
}

fn shell_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
}

fn read_hooks_json(path: &Path) -> Result<Value, AmuxError> {
    if !path.exists() {
        return Ok(json!({}));
    }

    let contents = fs::read_to_string(path)
        .map_err(|e| AmuxError::Setup(format!("failed to read hooks.json: {e}")))?;
    let parsed: Value = serde_json::from_str(&contents)
        .map_err(|e| AmuxError::Setup(format!("failed to parse hooks.json: {e}")))?;

    if parsed.is_object() {
        Ok(parsed)
    } else {
        Err(AmuxError::Setup(
            "expected ~/.codex/hooks.json to contain a JSON object".to_string(),
        ))
    }
}

fn command_is_managed(command: &str, installed_path: &Path) -> bool {
    let installed_path = installed_path.to_string_lossy();
    command.contains(installed_path.as_ref()) || command.contains(".codex/hooks/amux-status.sh")
}

fn remove_managed_hooks(definition: &Value, installed_path: &Path) -> Option<Value> {
    let Some(object) = definition.as_object() else {
        return Some(definition.clone());
    };

    let Some(hooks) = object.get("hooks").and_then(Value::as_array) else {
        return Some(definition.clone());
    };

    let filtered_hooks: Vec<Value> = hooks
        .iter()
        .filter(|hook| {
            !hook
                .get("command")
                .and_then(Value::as_str)
                .is_some_and(|command| command_is_managed(command, installed_path))
        })
        .cloned()
        .collect();

    if filtered_hooks.len() == hooks.len() {
        return Some(definition.clone());
    }

    if filtered_hooks.is_empty() {
        return None;
    }

    let mut cleaned = object.clone();
    cleaned.insert("hooks".to_string(), Value::Array(filtered_hooks));
    Some(Value::Object(cleaned))
}

fn merge_hooks_json(mut root: Value, installed_path: &Path) -> Value {
    if !root.get("hooks").is_some_and(Value::is_object) {
        root.as_object_mut()
            .expect("root is validated as object")
            .insert("hooks".to_string(), Value::Object(Map::new()));
    }

    let hooks = root
        .get_mut("hooks")
        .and_then(Value::as_object_mut)
        .expect("hooks is an object");

    for (_event_name, definitions) in hooks.iter_mut() {
        let Some(array) = definitions.as_array_mut() else {
            continue;
        };
        let filtered: Vec<Value> = array
            .iter()
            .filter_map(|definition| remove_managed_hooks(definition, installed_path))
            .collect();
        *array = filtered;
    }

    hooks.retain(|_, definitions| definitions.as_array().is_none_or(|array| !array.is_empty()));

    let command = format!(
        "{} || true",
        shell_quote(installed_path.to_string_lossy().as_ref())
    );

    for event_name in MANAGED_EVENTS {
        let definition = if event_requires_matcher(event_name) {
            json!({
                "matcher": "*",
                "hooks": [
                    {
                        "type": "command",
                        "command": command
                    }
                ]
            })
        } else {
            json!({
                "hooks": [
                    {
                        "type": "command",
                        "command": command
                    }
                ]
            })
        };

        match hooks.get_mut(*event_name).and_then(Value::as_array_mut) {
            Some(array) => array.push(definition),
            None => {
                hooks.insert((*event_name).to_string(), Value::Array(vec![definition]));
            }
        }
    }

    root
}

pub fn run() -> Result<(), AmuxError> {
    let script_path = hook_script_path()?;
    let hooks_path = hooks_json_path()?;
    let current_version = version_from_content(HOOK_SCRIPT)
        .ok_or_else(|| AmuxError::Setup("could not parse version from hook script".to_string()))?;

    let script_current = fs::read_to_string(&script_path)
        .ok()
        .is_some_and(|existing| version_from_content(&existing) == Some(current_version));

    if let Some(parent) = script_path.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| AmuxError::Setup(format!("failed to create hooks directory: {e}")))?;
    }

    if !script_current {
        fs::write(&script_path, HOOK_SCRIPT)
            .map_err(|e| AmuxError::Setup(format!("failed to write hook script: {e}")))?;
    }
    fs::set_permissions(&script_path, fs::Permissions::from_mode(0o755))
        .map_err(|e| AmuxError::Setup(format!("failed to set hook script permissions: {e}")))?;

    let existing = read_hooks_json(&hooks_path)?;
    let merged = merge_hooks_json(existing, &script_path);
    let content = serde_json::to_string_pretty(&merged)
        .map_err(|e| AmuxError::Setup(format!("failed to serialize hooks.json: {e}")))?
        + "\n";

    if let Some(parent) = hooks_path.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| AmuxError::Setup(format!("failed to create Codex directory: {e}")))?;
    }
    fs::write(&hooks_path, content)
        .map_err(|e| AmuxError::Setup(format!("failed to write hooks.json: {e}")))?;

    println!("Codex hook installed at {}", script_path.display());
    println!("Codex hooks configured at {}", hooks_path.display());

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::merge_hooks_json;
    use serde_json::json;
    use std::path::Path;

    #[test]
    fn merge_preserves_user_hooks_and_replaces_managed_hooks() {
        let path = Path::new("/Users/example/.codex/hooks/amux-status.sh");
        let existing = json!({
            "hooks": {
                "UserPromptSubmit": [
                    { "hooks": [{ "type": "command", "command": "echo user" }] },
                    { "hooks": [{ "type": "command", "command": "'/Users/example/.codex/hooks/amux-status.sh' || true" }] }
                ],
                "Stop": []
            },
            "other": true
        });

        let merged = merge_hooks_json(existing, path);
        let prompt_hooks = merged["hooks"]["UserPromptSubmit"].as_array().unwrap();
        assert_eq!(prompt_hooks.len(), 2);
        assert_eq!(prompt_hooks[0]["hooks"][0]["command"], json!("echo user"));
        assert_eq!(merged["other"], json!(true));
        assert!(merged["hooks"]["SessionStart"].is_array());
        assert!(merged["hooks"]["Stop"].is_array());
    }
}
