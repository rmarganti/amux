use serde::Deserialize;
use std;

use crate::agent::{AgentInstance, AgentProvider, AgentStatus};
use crate::error::AmuxError;
use crate::tmux::PaneInfo;

/// Agent provider for Gemini CLI instances.
pub struct GeminiProvider;

impl AgentProvider for GeminiProvider {
    fn name(&self) -> &'static str {
        "gemini"
    }

    fn discover(&self, panes: &[PaneInfo]) -> Result<Vec<AgentInstance>, AmuxError> {
        let mut instances = Vec::new();

        for pane in panes {
            if find_gemini_in_tree(pane.pane_pid).is_none() {
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
// Process Tree Walking
// ----------------------------------------------------------------

/// Walk the process tree rooted at `root_pid` to find a child that is
/// a Gemini CLI process. Returns the PID if found.
fn find_gemini_in_tree(root_pid: u32) -> Option<u32> {
    if is_gemini_process(root_pid) {
        return Some(root_pid);
    }

    let children = child_pids(root_pid);
    for child in children {
        if let Some(pid) = find_gemini_in_tree(child) {
            return Some(pid);
        }
    }

    None
}

/// Return the direct child PIDs of `pid`.
fn child_pids(pid: u32) -> Vec<u32> {
    let output = std::process::Command::new("pgrep")
        .args(["-P", &pid.to_string()])
        .output()
        .ok();

    let Some(output) = output else {
        return vec![];
    };

    String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter_map(|line| line.trim().parse::<u32>().ok())
        .collect()
}

/// Check whether `pid` is a Gemini CLI process.
///
/// - Basename equals `gemini` → SEA binary install.
/// - Basename is `node` and the full command args contain `gemini` → npm/dev install.
fn is_gemini_process(pid: u32) -> bool {
    let output = std::process::Command::new("ps")
        .args(["-p", &pid.to_string(), "-o", "comm="])
        .output()
        .ok();

    let Some(output) = output else {
        return false;
    };

    let comm = String::from_utf8_lossy(&output.stdout);
    let name = comm.trim();
    let basename = name.rsplit('/').next().unwrap_or(name);

    if basename == "gemini" {
        return true;
    }

    if basename == "node" {
        let args_output = std::process::Command::new("ps")
            .args(["-p", &pid.to_string(), "-o", "args="])
            .output()
            .ok();

        if let Some(args_output) = args_output {
            let args = String::from_utf8_lossy(&args_output.stdout);
            return args.contains("gemini");
        }
    }

    false
}

// ----------------------------------------------------------------
// Status file reading
// ----------------------------------------------------------------

/// Status file written by the amux-status Gemini CLI extension hook.
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
    .join("gemini")
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
