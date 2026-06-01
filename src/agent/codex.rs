use crate::agent::process_table::ProcessTable;
use crate::agent::{AgentInstance, AgentProvider, AgentStatus};
use crate::error::AmuxError;
use crate::tmux::PaneInfo;

/// Agent provider for Codex CLI instances.
pub struct CodexProvider;

fn is_codex_process(comm: &str, args: &str) -> bool {
    comm == "codex"
        || comm.starts_with("codex-")
        || args.split_whitespace().any(|arg| {
            arg.rsplit('/')
                .next()
                .is_some_and(|name| name == "codex" || name.starts_with("codex-"))
        })
}

impl AgentProvider for CodexProvider {
    fn name(&self) -> &'static str {
        "codex"
    }

    fn discover(
        &self,
        panes: &[PaneInfo],
        process_table: &ProcessTable,
    ) -> Result<Vec<AgentInstance>, AmuxError> {
        let mut instances = Vec::new();

        for pane in panes {
            let is_codex = process_table.has_process_in_tree(pane.pane_pid, &|process_info| {
                is_codex_process(&process_info.comm, &process_info.args)
            });

            if !is_codex {
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

#[cfg(test)]
mod tests {
    use super::is_codex_process;

    #[test]
    fn detects_codex_process_names_and_paths() {
        assert!(is_codex_process("codex", "codex"));
        assert!(is_codex_process("codex-darwin-arm64", "codex"));
        assert!(is_codex_process("node", "/opt/homebrew/bin/codex --foo"));
        assert!(is_codex_process("sh", "/tmp/codex-wrapper/codex-exec"));
        assert!(!is_codex_process("node", "/usr/bin/node other"));
    }
}
