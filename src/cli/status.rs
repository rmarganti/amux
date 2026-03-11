use crate::agent::process_table::ProcessTable;
use crate::agent::{self, AgentStatus};
use crate::error::AmuxError;
use crate::tmux::{self, SystemTmuxRunner};

/// Format aggregated agent statuses as a terse tmux-interpolatable string.
///
/// Output example: `#[fg=green]●2 #[fg=yellow]⚠1`
/// Only statuses with a non-zero count are included.
fn format_status_summary(counts: &StatusCounts) -> String {
    let mut parts: Vec<String> = Vec::new();

    if counts.running > 0 {
        parts.push(format!("#[fg=green]●{}", counts.running));
    }
    if counts.idle > 0 {
        parts.push(format!("#[default]○{}", counts.idle));
    }
    if counts.awaiting_input > 0 {
        parts.push(format!("#[fg=yellow]⚠{}", counts.awaiting_input));
    }
    if counts.errored > 0 {
        parts.push(format!("#[fg=red]✖{}", counts.errored));
    }

    if parts.is_empty() {
        return String::new();
    }

    parts.join(" ") + "#[default]"
}

#[derive(Debug, Default, PartialEq, Eq)]
struct StatusCounts {
    running: usize,
    idle: usize,
    awaiting_input: usize,
    errored: usize,
}

pub fn run() -> Result<(), AmuxError> {
    agent::status_file::purge_all_stale_files();

    let runner = SystemTmuxRunner;
    let panes = match tmux::list_panes(&runner) {
        Ok(p) => p,
        Err(_) => {
            print!("#[fg=red]⚠");
            return Ok(());
        }
    };

    let providers = agent::all_providers();
    let process_table = ProcessTable::snapshot();
    let mut counts = StatusCounts::default();

    // Run provider discovery in parallel. `std::thread::scope` guarantees all
    // spawned threads finish before the closure returns, so borrowed locals
    // (`panes`, `process_table`) are safe to share without `Arc`.
    std::thread::scope(|s| {
        let handles: Vec<_> = providers
            .iter()
            .map(|provider| s.spawn(|| provider.discover(&panes, &process_table)))
            .collect();

        for handle in handles {
            if let Ok(Ok(instances)) = handle.join() {
                for instance in &instances {
                    match instance.status {
                        AgentStatus::Running => counts.running += 1,
                        AgentStatus::Idle => counts.idle += 1,
                        AgentStatus::AwaitingInput => counts.awaiting_input += 1,
                        AgentStatus::Errored => counts.errored += 1,
                    }
                }
            }
        }
    });

    print!("{}", format_status_summary(&counts));

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_status_summary_all_statuses() {
        let counts = StatusCounts {
            running: 2,
            idle: 1,
            awaiting_input: 1,
            errored: 1,
        };
        assert_eq!(
            format_status_summary(&counts),
            "#[fg=green]●2 #[default]○1 #[fg=yellow]⚠1 #[fg=red]✖1#[default]"
        );
    }

    #[test]
    fn test_format_status_summary_only_running() {
        let counts = StatusCounts {
            running: 3,
            ..Default::default()
        };
        assert_eq!(format_status_summary(&counts), "#[fg=green]●3#[default]");
    }

    #[test]
    fn test_format_status_summary_only_awaiting() {
        let counts = StatusCounts {
            awaiting_input: 2,
            ..Default::default()
        };
        assert_eq!(format_status_summary(&counts), "#[fg=yellow]⚠2#[default]");
    }

    #[test]
    fn test_format_status_summary_empty() {
        let counts = StatusCounts::default();
        assert_eq!(format_status_summary(&counts), "");
    }

    #[test]
    fn test_format_status_summary_mixed() {
        let counts = StatusCounts {
            running: 1,
            errored: 2,
            ..Default::default()
        };
        assert_eq!(
            format_status_summary(&counts),
            "#[fg=green]●1 #[fg=red]✖2#[default]"
        );
    }
}
