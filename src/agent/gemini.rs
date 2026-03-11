use crate::agent::process_table::ProcessTable;
use crate::agent::status_file;
use crate::agent::{AgentInstance, AgentProvider, AgentStatus};
use crate::error::AmuxError;
use crate::tmux::PaneInfo;

/// Agent provider for Gemini CLI instances.
pub struct GeminiProvider;

impl AgentProvider for GeminiProvider {
    fn name(&self) -> &'static str {
        "gemini"
    }

    fn discover(
        &self,
        panes: &[PaneInfo],
        process_table: &ProcessTable,
    ) -> Result<Vec<AgentInstance>, AmuxError> {
        let mut instances = Vec::new();

        for pane in panes {
            let is_gemini = process_table.has_process_in_tree(pane.pane_pid, &|process_info| {
                if process_info.comm == "gemini" {
                    return true;
                }
                process_info.comm == "node" && process_info.args.contains("gemini")
            });

            if !is_gemini {
                continue;
            }

            let status = status_file::read_status_file("gemini", &pane.pane_id, |s| match s {
                "busy" => Some(AgentStatus::Running),
                "idle" => Some(AgentStatus::Idle),
                "awaiting_input" => Some(AgentStatus::AwaitingInput),
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
