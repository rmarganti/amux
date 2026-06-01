use crate::agent::process_table::ProcessTable;
use crate::agent::{AgentInstance, AgentProvider, AgentStatus};
use crate::error::AmuxError;
use crate::tmux::PaneInfo;

/// Agent provider for Amp instances.
pub struct AmpProvider;

impl AgentProvider for AmpProvider {
    fn name(&self) -> &'static str {
        "amp"
    }

    fn discover(
        &self,
        panes: &[PaneInfo],
        process_table: &ProcessTable,
    ) -> Result<Vec<AgentInstance>, AmuxError> {
        let mut instances = Vec::new();

        for pane in panes {
            let is_amp = process_table
                .has_process_in_tree(pane.pane_pid, &|process_info| process_info.comm == "amp");

            if !is_amp {
                continue;
            }

            instances.push(AgentInstance {
                pane: pane.clone(),
                provider_name: self.name(),
                status: AgentStatus::Idle,
            });
        }

        Ok(instances)
    }
}
