use serde::Deserialize;
use std::path::{Path, PathBuf};

use crate::agent::AgentStatus;

/// Status file written by agent plugins.
#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct StatusFile {
    pub provider: String,
    pub status: String,
    pub pid: u32,
    pub ts: u64,
}

/// Maximum age (in seconds) before a status file is considered stale unless
/// its recorded PID is still alive.
const STALE_THRESHOLD_SECS: u64 = 30;

/// Providers allowed to claim a shared pane status file.
const KNOWN_PROVIDERS: &[&str] = &["amp", "codex", "gemini", "opencode", "pi"];

/// Return the base directory for all agent status files
/// (`$XDG_STATE_HOME/amux/`, or `~/.local/state/amux/`).
pub fn status_base_dir() -> PathBuf {
    if let Ok(xdg) = std::env::var("XDG_STATE_HOME") {
        PathBuf::from(xdg)
    } else {
        dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("/tmp"))
            .join(".local")
            .join("state")
    }
    .join("amux")
}

/// Return the shared status file path for a tmux pane.
pub fn status_file_path(pane_id: &str) -> PathBuf {
    status_base_dir().join(format!("{pane_id}.json"))
}

/// Read, validate, and return the shared status file for a tmux pane.
///
/// Returns `None` if the file is missing, unparseable, uses an unknown provider
/// or status, or is stale (older than 30 seconds with no matching live PID).
/// Stale and unparseable files are removed.
pub fn read_status_file(pane_id: &str) -> Option<StatusFile> {
    read_status_file_at(&status_file_path(pane_id))
}

fn read_status_file_at(path: &Path) -> Option<StatusFile> {
    let contents = std::fs::read_to_string(path).ok()?;
    let file: StatusFile = match serde_json::from_str(&contents) {
        Ok(file) => file,
        Err(_) => {
            let _ = std::fs::remove_file(path);
            return None;
        }
    };

    if !is_known_provider(&file.provider) || normalized_status(&file.status).is_none() {
        return None;
    }

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .ok()?
        .as_secs();

    if now.saturating_sub(file.ts) > STALE_THRESHOLD_SECS && !is_pid_alive(file.pid) {
        let _ = std::fs::remove_file(path);
        return None;
    }

    Some(file)
}

/// Map the normalized plugin status vocabulary to an `AgentStatus`.
pub fn normalized_status(status: &str) -> Option<AgentStatus> {
    match status {
        "running" => Some(AgentStatus::Running),
        "idle" => Some(AgentStatus::Idle),
        "awaiting_input" => Some(AgentStatus::AwaitingInput),
        "errored" => Some(AgentStatus::Errored),
        _ => None,
    }
}

/// Return whether a provider name is allowed in a status file.
pub fn is_known_provider(provider: &str) -> bool {
    KNOWN_PROVIDERS.contains(&provider)
}

/// Check whether a process with the given PID is still alive.
pub fn is_pid_alive(pid: u32) -> bool {
    std::process::Command::new("kill")
        .args(["-0", &pid.to_string()])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .is_ok_and(|s| s.success())
}

/// Remove shared status files whose recorded PID is no longer alive, plus
/// unparseable files.
pub fn purge_stale_files() {
    let dir = status_base_dir();
    let Ok(entries) = std::fs::read_dir(&dir) else {
        return;
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("json") {
            continue;
        }
        let Ok(contents) = std::fs::read_to_string(&path) else {
            continue;
        };
        let Ok(file) = serde_json::from_str::<StatusFile>(&contents) else {
            let _ = std::fs::remove_file(&path);
            continue;
        };
        if !is_known_provider(&file.provider)
            || normalized_status(&file.status).is_none()
            || !is_pid_alive(file.pid)
        {
            let _ = std::fs::remove_file(&path);
        }
    }
}

/// Purge stale shared status files.
pub fn purge_all_stale_files() {
    purge_stale_files();
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn status_json(pid: u32, status: &str) -> String {
        serde_json::json!({
            "provider": "opencode",
            "status": status,
            "pid": pid,
            "ts": 1000
        })
        .to_string()
    }

    #[test]
    fn maps_normalized_statuses() {
        assert_eq!(normalized_status("running"), Some(AgentStatus::Running));
        assert_eq!(normalized_status("idle"), Some(AgentStatus::Idle));
        assert_eq!(
            normalized_status("awaiting_input"),
            Some(AgentStatus::AwaitingInput)
        );
        assert_eq!(normalized_status("errored"), Some(AgentStatus::Errored));
        assert_eq!(normalized_status("busy"), None);
    }

    #[test]
    fn purge_stale_files_removes_dead_pid() {
        let tmp = tempfile::tempdir().unwrap();
        let dead_pid = 2_000_000_000u32;
        let file = tmp.path().join("%1.json");
        fs::write(&file, status_json(dead_pid, "idle")).unwrap();

        for entry in fs::read_dir(tmp.path()).unwrap().flatten() {
            let path = entry.path();
            let contents = fs::read_to_string(&path).unwrap();
            let file: StatusFile = serde_json::from_str(&contents).unwrap();
            if !is_pid_alive(file.pid) {
                let _ = fs::remove_file(&path);
            }
        }

        assert!(!file.exists(), "stale file should be removed");
    }

    #[test]
    fn purge_stale_files_keeps_live_pid() {
        let tmp = tempfile::tempdir().unwrap();
        let live_pid = std::process::id();
        let file = tmp.path().join("%2.json");
        fs::write(&file, status_json(live_pid, "running")).unwrap();

        for entry in fs::read_dir(tmp.path()).unwrap().flatten() {
            let path = entry.path();
            let contents = fs::read_to_string(&path).unwrap();
            let file: StatusFile = serde_json::from_str(&contents).unwrap();
            if !is_pid_alive(file.pid) {
                let _ = fs::remove_file(&path);
            }
        }

        assert!(file.exists(), "live pid file should be kept");
    }

    #[test]
    fn purge_stale_files_removes_unparseable() {
        let tmp = tempfile::tempdir().unwrap();
        let file = tmp.path().join("%3.json");
        fs::write(&file, "not valid json").unwrap();

        for entry in fs::read_dir(tmp.path()).unwrap().flatten() {
            let path = entry.path();
            let contents = fs::read_to_string(&path).unwrap();
            if serde_json::from_str::<StatusFile>(&contents).is_err() {
                let _ = fs::remove_file(&path);
            }
        }

        assert!(!file.exists(), "unparseable file should be removed");
    }

    #[test]
    fn purge_stale_files_ignores_non_json() {
        let tmp = tempfile::tempdir().unwrap();
        let file = tmp.path().join("notes.txt");
        fs::write(&file, "not a status file").unwrap();

        for entry in fs::read_dir(tmp.path()).unwrap().flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("json") {
                continue;
            }
        }

        assert!(file.exists(), "non-json file should be left alone");
    }
}
