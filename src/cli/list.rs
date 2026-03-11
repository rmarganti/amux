use std::io::Write;
use std::process::{Command, Stdio};

use crate::agent::process_table::ProcessTable;
use crate::agent::{self, AgentInstance, AgentStatus};
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

pub fn run(status_filter: Option<AgentStatus>, plain: bool) -> Result<(), AmuxError> {
    agent::status_file::purge_all_stale_files();

    if !tmux::is_inside_tmux() {
        return Err(AmuxError::NotInTmux);
    }

    let runner = SystemTmuxRunner;
    let panes = tmux::list_panes(&runner)?;
    let providers = agent::all_providers();
    let process_table = ProcessTable::snapshot();
    let mut instances: Vec<AgentInstance> = Vec::new();

    // Run provider discovery in parallel. `std::thread::scope` guarantees all
    // spawned threads finish before the closure returns, so borrowed locals
    // (`panes`, `process_table`) are safe to share without `Arc`.
    std::thread::scope(|s| {
        let handles: Vec<_> = providers
            .iter()
            .map(|provider| s.spawn(|| provider.discover(&panes, &process_table)))
            .collect();

        for handle in handles {
            if let Ok(result) = handle.join() {
                instances.extend(result?);
            }
        }

        Ok::<(), AmuxError>(())
    })?;

    if let Some(filter) = status_filter {
        instances.retain(|inst| inst.status == filter);
    }

    if instances.is_empty() {
        if status_filter.is_some() {
            // When a filter is active, output nothing so fzf shows an empty
            // list. The user can switch filters via keybinds.
            return Ok(());
        }
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

    if plain {
        println!("{fzf_input}");
        return Ok(());
    }

    let mut fzf = Command::new("fzf")
        .args([
            "--ansi",
            "--no-multi",
            "--delimiter",
            "\t",
            "--with-nth",
            "1",
            "--prompt",
            "all> ",
            "--header",
            "  ^a:all / ^r:running / ^i:idle / ^w:awaiting / ^e:errored",
            "--bind",
            "ctrl-a:change-prompt(all> )+reload(amux list --plain)",
            "--bind",
            "ctrl-r:change-prompt(running> )+reload(amux list --plain --status running)",
            "--bind",
            "ctrl-i:change-prompt(idle> )+reload(amux list --plain --status idle)",
            "--bind",
            "ctrl-w:change-prompt(awaiting> )+reload(amux list --plain --status awaiting-input)",
            "--bind",
            "ctrl-e:change-prompt(errored> )+reload(amux list --plain --status errored)",
            "--preview",
            "tmux capture-pane -e -p -t {2}",
            "--preview-window",
            "right:55%",
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
