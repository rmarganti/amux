use std::collections::HashMap;

use serde::Deserialize;

use crate::agent::{AgentInstance, AgentProvider, AgentStatus};
use crate::error::AmuxError;
use crate::tmux::PaneInfo;

/// Agent provider for OpenCode instances.
pub struct OpenCodeProvider;

impl AgentProvider for OpenCodeProvider {
    fn name(&self) -> &'static str {
        "opencode"
    }

    fn discover(&self, panes: &[PaneInfo]) -> Result<Vec<AgentInstance>, AmuxError> {
        let mut instances = Vec::new();

        for pane in panes {
            let Some(oc_pid) = find_opencode_in_tree(pane.pane_pid) else {
                continue;
            };

            let status = match discover_port(oc_pid) {
                Some(port) => {
                    let cwd = process_cwd(oc_pid).unwrap_or_default();
                    query_status(port, &cwd).unwrap_or(AgentStatus::Idle)
                }
                None => AgentStatus::Idle,
            };

            instances.push(AgentInstance {
                pane: pane.clone(),
                provider_name: self.name(),
                status,
            });
        }

        Ok(instances)
    }
}

// ── Process tree walking ──────────────────────────────────────────────

/// Walk the process tree rooted at `root_pid` to find a child whose
/// executable name starts with "opencode". Returns the PID if found.
fn find_opencode_in_tree(root_pid: u32) -> Option<u32> {
    if is_opencode_process(root_pid) {
        return Some(root_pid);
    }

    let children = child_pids(root_pid);
    for child in children {
        if let Some(pid) = find_opencode_in_tree(child) {
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

/// Check whether `pid` is an opencode process by inspecting its name.
fn is_opencode_process(pid: u32) -> bool {
    let output = std::process::Command::new("ps")
        .args(["-p", &pid.to_string(), "-o", "comm="])
        .output()
        .ok();

    let Some(output) = output else {
        return false;
    };

    let comm = String::from_utf8_lossy(&output.stdout);
    let name = comm.trim();

    // Match both `opencode` (Homebrew) and `opencode-*` (npm/bun).
    // `comm` may include the full path, so check the basename.
    let basename = name.rsplit('/').next().unwrap_or(name);
    basename == "opencode" || basename.starts_with("opencode-")
}

// ── Port discovery ────────────────────────────────────────────────────

/// Discover the TCP port the opencode process is listening on via `lsof`.
fn discover_port(pid: u32) -> Option<u16> {
    let output = std::process::Command::new("lsof")
        .args(["-iTCP", "-sTCP:LISTEN", "-a", "-p", &pid.to_string(), "-Fn"])
        .output()
        .ok()?;

    let stdout = String::from_utf8_lossy(&output.stdout);

    // lsof -Fn outputs lines like:
    //   p<pid>
    //   n*:<port>
    //   n127.0.0.1:<port>
    for line in stdout.lines() {
        let line = line.trim();
        if let Some(rest) = line.strip_prefix('n') {
            if let Some(port_str) = rest.rsplit(':').next() {
                if let Ok(port) = port_str.parse::<u16>() {
                    return Some(port);
                }
            }
        }
    }

    None
}

// ── Working directory detection ───────────────────────────────────────

/// Read the current working directory of a process.
fn process_cwd(pid: u32) -> Option<String> {
    // On macOS, `lsof -a -p <pid> -d cwd -Fn` outputs the cwd path.
    // On Linux, we could read /proc/<pid>/cwd, but lsof works on both.
    let output = std::process::Command::new("lsof")
        .args(["-a", "-p", &pid.to_string(), "-d", "cwd", "-Fn"])
        .output()
        .ok()?;

    let stdout = String::from_utf8_lossy(&output.stdout);

    // Output lines look like:
    //   p<pid>
    //   fcwd
    //   n/path/to/dir
    for line in stdout.lines() {
        let line = line.trim();
        if let Some(path) = line.strip_prefix('n') {
            if path.starts_with('/') {
                return Some(path.to_string());
            }
        }
    }

    None
}

// ── Status detection ──────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
struct SessionStatus {
    #[serde(rename = "type")]
    status_type: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PermissionRequest {
    session_id: String,
}

/// Query the OpenCode HTTP API to determine the agent status.
///
/// 1. `GET /session/status` → map of session ID → status
/// 2. `GET /permission` → list of pending permission requests
///
/// Mapping:
///   - Any session `busy` with pending permissions → AwaitingInput
///   - Any session `busy` → Running
///   - Any session `retry` → Errored
///   - Otherwise → Idle
fn query_status(port: u16, directory: &str) -> Result<AgentStatus, AmuxError> {
    let base = format!("http://127.0.0.1:{port}");

    let statuses: HashMap<String, SessionStatus> = http_get_json(
        &format!("{base}/session/status"),
        directory,
    )?;

    let permissions: Vec<PermissionRequest> = http_get_json(
        &format!("{base}/permission"),
        directory,
    )?;

    // Build a set of session IDs that have pending permissions.
    let pending_sessions: std::collections::HashSet<&str> = permissions
        .iter()
        .map(|p| p.session_id.as_str())
        .collect();

    let mut has_busy = false;
    let mut has_retry = false;

    for (session_id, status) in &statuses {
        match status.status_type.as_str() {
            "busy" => {
                if pending_sessions.contains(session_id.as_str()) {
                    return Ok(AgentStatus::AwaitingInput);
                }
                has_busy = true;
            }
            "retry" => {
                has_retry = true;
            }
            _ => {}
        }
    }

    if has_busy {
        return Ok(AgentStatus::Running);
    }

    if has_retry {
        return Ok(AgentStatus::Errored);
    }

    Ok(AgentStatus::Idle)
}

fn http_get_json<T: serde::de::DeserializeOwned>(
    url: &str,
    directory: &str,
) -> Result<T, AmuxError> {
    let response = ureq::get(url)
        .header("x-opencode-directory", directory)
        .call()
        .map_err(|e| AmuxError::Provider(format!("HTTP request to {url} failed: {e}")))?;

    let body = response
        .into_body()
        .read_to_string()
        .map_err(|e| AmuxError::Provider(format!("failed to read response body: {e}")))?;

    serde_json::from_str(&body)
        .map_err(|e| AmuxError::Provider(format!("failed to parse JSON from {url}: {e}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_find_opencode_in_tree_returns_none_for_nonexistent_pid() {
        // PID 0 should never be an opencode process.
        assert!(find_opencode_in_tree(0).is_none());
    }

    #[test]
    fn test_child_pids_returns_empty_for_nonexistent_pid() {
        assert!(child_pids(0).is_empty());
    }

    #[test]
    fn test_is_opencode_process_returns_false_for_init() {
        // PID 1 (launchd/init) is not opencode.
        assert!(!is_opencode_process(1));
    }

    #[test]
    fn test_discover_port_returns_none_for_nonexistent_pid() {
        assert!(discover_port(0).is_none());
    }

    #[test]
    fn test_process_cwd_returns_none_for_nonexistent_pid() {
        assert!(process_cwd(0).is_none());
    }

    #[test]
    fn test_opencode_provider_discover_no_panes() {
        let provider = OpenCodeProvider;
        let result = provider.discover(&[]).unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn test_opencode_provider_discover_no_opencode_panes() {
        let provider = OpenCodeProvider;
        let panes = vec![PaneInfo {
            session_name: "test".to_string(),
            window_name: "shell".to_string(),
            pane_id: "%0".to_string(),
            pane_pid: 1, // init/launchd — not opencode
        }];
        let result = provider.discover(&panes).unwrap();
        assert!(result.is_empty());
    }
}
