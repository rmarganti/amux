use crate::agent::process_table::ProcessTable;
use crate::agent::{self, AgentInstance, AgentStatus};
use crate::error::AmuxError;
use crate::tmux::{self, SystemTmuxRunner};

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
            // Each thread returns a `Result<Vec<AgentInstance>, AmuxError>`, so we need to
            // unwrap both the thread result and the provider discovery result.
            if let Ok(Ok(mut discovered)) = handle.join() {
                instances.append(&mut discovered);
            }
        }
    });

    print!("{}", format_status_summary(&instances));

    Ok(())
}

/// Format agent statuses as colored icons.
///
/// Output example: `#[fg=green]● #[default]○ #[fg=yellow]⚠#[default]`
fn format_status_summary(instances: &[AgentInstance]) -> String {
    if instances.is_empty() {
        return String::new();
    }

    let icons: Vec<String> = instances
        .iter()
        .map(|instance| status_to_icon(&instance.status).to_string())
        .collect();

    icons.join(" ") + "#[default]"
}

/// Get the icon and color for a given agent status.
fn status_to_icon(status: &AgentStatus) -> &'static str {
    match status {
        AgentStatus::Running => "#[fg=green]●",
        AgentStatus::Idle => "#[default]○",
        AgentStatus::AwaitingInput => "#[fg=yellow]⚠",
        AgentStatus::Errored => "#[fg=red]✖",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tmux::PaneInfo;

    fn create_instance(status: AgentStatus) -> AgentInstance {
        AgentInstance {
            pane: PaneInfo {
                session_name: "test".to_string(),
                window_name: "0".to_string(),
                pane_id: "0".to_string(),
                pane_pid: 0,
            },
            provider_name: "test",
            status,
        }
    }

    #[test]
    fn test_format_status_summary_all_statuses() {
        let instances = vec![
            create_instance(AgentStatus::Running),
            create_instance(AgentStatus::Running),
            create_instance(AgentStatus::Idle),
            create_instance(AgentStatus::AwaitingInput),
            create_instance(AgentStatus::Errored),
        ];
        assert_eq!(
            format_status_summary(&instances),
            "#[fg=green]● #[fg=green]● #[default]○ #[fg=yellow]⚠ #[fg=red]✖#[default]"
        );
    }

    #[test]
    fn test_format_status_summary_only_running() {
        let instances = vec![
            create_instance(AgentStatus::Running),
            create_instance(AgentStatus::Running),
            create_instance(AgentStatus::Running),
        ];
        assert_eq!(
            format_status_summary(&instances),
            "#[fg=green]● #[fg=green]● #[fg=green]●#[default]"
        );
    }

    #[test]
    fn test_format_status_summary_only_awaiting() {
        let instances = vec![
            create_instance(AgentStatus::AwaitingInput),
            create_instance(AgentStatus::AwaitingInput),
        ];
        assert_eq!(
            format_status_summary(&instances),
            "#[fg=yellow]⚠ #[fg=yellow]⚠#[default]"
        );
    }

    #[test]
    fn test_format_status_summary_empty() {
        let instances: Vec<AgentInstance> = vec![];
        assert_eq!(format_status_summary(&instances), "");
    }

    #[test]
    fn test_format_status_summary_mixed() {
        let instances = vec![
            create_instance(AgentStatus::Running),
            create_instance(AgentStatus::Errored),
            create_instance(AgentStatus::Errored),
        ];
        assert_eq!(
            format_status_summary(&instances),
            "#[fg=green]● #[fg=red]✖ #[fg=red]✖#[default]"
        );
    }
}
