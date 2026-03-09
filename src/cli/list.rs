use std::io::Write;
use std::process::{Command, Stdio};

use crate::agent::{self, AgentInstance};
use crate::error::AmuxError;
use crate::tmux::{self, SystemTmuxRunner};

/// Format a single agent instance for display in fzf.
/// Format: `[session > window] agent - status`
fn format_line(instance: &AgentInstance) -> String {
    format!(
        "[{} > {}] {} - {}",
        instance.pane.session_name,
        instance.pane.window_name,
        instance.provider_name,
        instance.status,
    )
}

pub fn run() -> Result<(), AmuxError> {
    if !tmux::is_inside_tmux() {
        return Err(AmuxError::NotInTmux);
    }

    let runner = SystemTmuxRunner;
    let panes = tmux::list_panes(&runner)?;
    let providers = agent::all_providers();

    let mut instances: Vec<AgentInstance> = Vec::new();
    for provider in &providers {
        instances.extend(provider.discover(&panes)?);
    }

    if instances.is_empty() {
        return Err(AmuxError::NoAgentsFound);
    }

    // Each fzf line: `<display>\t<pane_id>`
    // We use --delimiter='\t' and --with-nth=1 so fzf only shows the display
    // portion, but the full line (including pane_id) is returned on selection.
    let fzf_input: String = instances
        .iter()
        .map(|inst| format!("{}\t{}", format_line(inst), inst.pane.pane_id))
        .collect::<Vec<_>>()
        .join("\n");

    let mut fzf = Command::new("fzf")
        .args([
            "--ansi",
            "--no-multi",
            "--delimiter",
            "\t",
            "--with-nth",
            "1",
            "--prompt",
            "agent> ",
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()
        .map_err(|e| AmuxError::Fzf(format!("failed to start fzf: {e}")))?;

    if let Some(ref mut stdin) = fzf.stdin {
        let _ = stdin.write_all(fzf_input.as_bytes());
    }
    // Drop stdin so fzf sees EOF.
    drop(fzf.stdin.take());

    let output = fzf
        .wait_with_output()
        .map_err(|e| AmuxError::Fzf(format!("fzf failed: {e}")))?;

    if !output.status.success() {
        return Err(AmuxError::FzfNoSelection);
    }

    let selection = String::from_utf8_lossy(&output.stdout);
    let selection = selection.trim();

    // The pane_id is after the tab character.
    let pane_id = selection
        .rsplit('\t')
        .next()
        .ok_or(AmuxError::FzfNoSelection)?;

    let instance = instances
        .iter()
        .find(|inst| inst.pane.pane_id == pane_id)
        .ok_or(AmuxError::FzfNoSelection)?;

    tmux::switch_to_pane(&runner, &instance.pane)?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::AgentStatus;
    use crate::tmux::PaneInfo;

    #[test]
    fn test_format_line() {
        let instance = AgentInstance {
            pane: PaneInfo {
                session_name: "work".to_string(),
                window_name: "code".to_string(),
                pane_id: "%3".to_string(),
                pane_pid: 100,
            },
            provider_name: "opencode",
            status: AgentStatus::Running,
        };
        assert_eq!(format_line(&instance), "[work > code] opencode - running");
    }

    #[test]
    fn test_format_line_awaiting_input() {
        let instance = AgentInstance {
            pane: PaneInfo {
                session_name: "dev".to_string(),
                window_name: "agent".to_string(),
                pane_id: "%1".to_string(),
                pane_pid: 200,
            },
            provider_name: "opencode",
            status: AgentStatus::AwaitingInput,
        };
        assert_eq!(
            format_line(&instance),
            "[dev > agent] opencode - awaiting input"
        );
    }
}
