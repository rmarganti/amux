use crate::agent::process_table::ProcessTable;
use crate::agent::status_file;
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

            let status = status_file::read_status_file("opencode", &pane.pane_id, |s| match s {
                "busy" => Some(AgentStatus::Running),
                "idle" => Some(AgentStatus::Idle),
                "awaiting_input" => Some(AgentStatus::AwaitingInput),
                "errored" => Some(AgentStatus::Errored),
                _ => None,
            })
            .unwrap_or(AgentStatus::Idle);

            instances.push(AgentInstance {
                pane: pane.clone(),
                provider_name: self.name(),
                status,
            });
        }

        Ok(instances)
    }
}
