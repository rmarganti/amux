use serde::Deserialize;
use std;

use crate::agent::process_table::ProcessTable;
use crate::agent::{AgentInstance, AgentProvider, AgentStatus};
use crate::error::AmuxError;
use crate::tmux::PaneInfo;

/// Agent provider for OpenCode instances.
pub struct OpenCodeProvider;

impl AgentProvider for OpenCodeProvider {
    fn name(&self) -> &'static str {
        "opencode"
    }

    fn discover(
        &self,
        panes: &[PaneInfo],
        process_table: &ProcessTable,
    ) -> Result<Vec<AgentInstance>, AmuxError> {
        let mut instances = Vec::new();

        for pane in panes {
            let is_opencode = process_table.has_process_in_tree(pane.pane_pid, &|process_info| {
                process_info.comm == "opencode" || process_info.comm.starts_with("opencode-")
            });

            if !is_opencode {
                continue;
            }

            let status = read_status_file(&pane.pane_id).unwrap_or(AgentStatus::Idle);

            instances.push(AgentInstance {
                pane: pane.clone(),
                provider_name: self.name(),
                status,
            });
        }

        Ok(instances)
    }
}

// ----------------------------------------------------------------
// Status file reading
// ----------------------------------------------------------------

/// Status file written by the amux-status OpenCode plugin.
#[derive(Debug, Deserialize)]
struct StatusFile {
    status: String,
    pid: u32,
    ts: u64,
}

/// Maximum age (in seconds) before a status file is considered stale.
const STALE_THRESHOLD_SECS: u64 = 30;

/// Return the directory containing per-pane status files.
fn status_dir() -> std::path::PathBuf {
    if let Ok(xdg) = std::env::var("XDG_STATE_HOME") {
        std::path::PathBuf::from(xdg)
    } else {
        dirs::home_dir()
            .unwrap_or_else(|| std::path::PathBuf::from("/tmp"))
            .join(".local")
            .join("state")
    }
    .join("amux")
    .join("opencode")
}

/// Read the status file for a given pane and map it to an `AgentStatus`.
///
/// Returns `None` if the file is missing, unparseable, or stale (timestamp
/// older than 30 s with no matching live PID). Stale files are cleaned up.
fn read_status_file(pane_id: &str) -> Option<AgentStatus> {
    let path = status_dir().join(format!("{pane_id}.json"));
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

    match file.status.as_str() {
        "busy" => Some(AgentStatus::Running),
        "idle" => Some(AgentStatus::Idle),
        "awaiting_input" => Some(AgentStatus::AwaitingInput),
        "errored" => Some(AgentStatus::Errored),
        _ => None,
    }
}

/// Check whether a process with the given PID is still alive.
fn is_pid_alive(pid: u32) -> bool {
    std::process::Command::new("kill")
        .args(["-0", &pid.to_string()])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .is_ok_and(|s| s.success())
}
