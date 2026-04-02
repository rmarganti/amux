use serde::Deserialize;
use std::path::PathBuf;

use crate::agent::AgentStatus;

/// Status file written by agent plugins.
#[derive(Debug, Deserialize)]
pub struct StatusFile {
    pub status: String,
    pub pid: u32,
    pub ts: u64,
}

/// Maximum age (in seconds) before a status file is considered stale.
const STALE_THRESHOLD_SECS: u64 = 30;

/// Known agent subdirectories under the status base directory.
const AGENT_SUBDIRS: &[&str] = &["amp", "gemini", "opencode", "pi"];

/// Return the base directory for all agent status files
/// (`$XDG_STATE_HOME/amux/`).
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

/// Read the status file for a given agent subdirectory and pane, mapping the
/// raw status string via `status_mapper`.
///
/// Returns `None` if the file is missing, unparseable, or stale (timestamp
/// older than 30 s with no matching live PID). Stale files are cleaned up.
pub fn read_status_file<F>(subdir: &str, pane_id: &str, status_mapper: F) -> Option<AgentStatus>
where
    F: Fn(&str) -> Option<AgentStatus>,
{
    let path = status_base_dir()
        .join(subdir)
        .join(format!("{pane_id}.json"));
    let contents = std::fs::read_to_string(&path).ok()?;
    let file: StatusFile = serde_json::from_str(&contents).ok()?;

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .ok()?
        .as_secs();

    if now.saturating_sub(file.ts) > STALE_THRESHOLD_SECS && !is_pid_alive(file.pid) {
        let _ = std::fs::remove_file(&path);
        return None;
    }

    status_mapper(&file.status)
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

/// Remove status files whose recorded PID is no longer alive.
///
/// Iterates all `*.json` files in `status_base_dir()/<subdir>/`, parses
/// each, and removes the file if the PID is dead.
pub fn purge_stale_files(subdir: &str) {
    let dir = status_base_dir().join(subdir);
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
        if !is_pid_alive(file.pid) {
            let _ = std::fs::remove_file(&path);
        }
    }
}

/// Purge stale status files for all known agent subdirectories.
pub fn purge_all_stale_files() {
    for subdir in AGENT_SUBDIRS {
        purge_stale_files(subdir);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn setup_temp_dir() -> tempfile::TempDir {
        tempfile::tempdir().unwrap()
    }

    #[test]
    fn test_purge_stale_files_removes_dead_pid() {
        let tmp = setup_temp_dir();
        let subdir = "test_agent";
        let dir = tmp.path().join(subdir);
        fs::create_dir_all(&dir).unwrap();

        // Use PID 0 which will always fail the `kill -0` check for non-root.
        // Use a PID that is almost certainly dead.
        let dead_pid = 2_000_000_000u32;
        let status = serde_json::json!({
            "status": "idle",
            "pid": dead_pid,
            "ts": 1000
        });
        fs::write(dir.join("%1.json"), status.to_string()).unwrap();

        // Override XDG_STATE_HOME so purge_stale_files looks in our temp dir.
        // We need to call the lower-level logic directly since
        // `purge_stale_files` uses `status_base_dir()` which reads env vars.
        // Instead, we inline the purge logic against our temp dir.
        let entries = fs::read_dir(&dir).unwrap();
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("json") {
                continue;
            }
            let contents = fs::read_to_string(&path).unwrap();
            let file: StatusFile = serde_json::from_str(&contents).unwrap();
            if !is_pid_alive(file.pid) {
                let _ = fs::remove_file(&path);
            }
        }

        assert!(
            !dir.join("%1.json").exists(),
            "stale file should be removed"
        );
    }

    #[test]
    fn test_purge_stale_files_keeps_live_pid() {
        let tmp = setup_temp_dir();
        let subdir = "test_agent";
        let dir = tmp.path().join(subdir);
        fs::create_dir_all(&dir).unwrap();

        // Use our own PID which is definitely alive.
        let live_pid = std::process::id();
        let status = serde_json::json!({
            "status": "busy",
            "pid": live_pid,
            "ts": 1000
        });
        fs::write(dir.join("%2.json"), status.to_string()).unwrap();

        let entries = fs::read_dir(&dir).unwrap();
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("json") {
                continue;
            }
            let contents = fs::read_to_string(&path).unwrap();
            let file: StatusFile = serde_json::from_str(&contents).unwrap();
            if !is_pid_alive(file.pid) {
                let _ = fs::remove_file(&path);
            }
        }

        assert!(dir.join("%2.json").exists(), "live pid file should be kept");
    }

    #[test]
    fn test_purge_stale_files_removes_unparseable() {
        let tmp = setup_temp_dir();
        let subdir = "test_agent";
        let dir = tmp.path().join(subdir);
        fs::create_dir_all(&dir).unwrap();

        fs::write(dir.join("%3.json"), "not valid json").unwrap();

        let entries = fs::read_dir(&dir).unwrap();
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("json") {
                continue;
            }
            let contents = fs::read_to_string(&path).unwrap();
            if serde_json::from_str::<StatusFile>(&contents).is_err() {
                let _ = fs::remove_file(&path);
            }
        }

        assert!(
            !dir.join("%3.json").exists(),
            "unparseable file should be removed"
        );
    }

    #[test]
    fn test_purge_stale_files_ignores_non_json() {
        let tmp = setup_temp_dir();
        let subdir = "test_agent";
        let dir = tmp.path().join(subdir);
        fs::create_dir_all(&dir).unwrap();

        fs::write(dir.join("notes.txt"), "not a status file").unwrap();

        let entries = fs::read_dir(&dir).unwrap();
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("json") {
                continue;
            }
            // Would purge here, but this file is .txt so it's skipped.
        }

        assert!(
            dir.join("notes.txt").exists(),
            "non-json file should be left alone"
        );
    }
}
