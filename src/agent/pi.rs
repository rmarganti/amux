use crate::agent::process_table::ProcessTable;
use crate::agent::status_file;
use crate::agent::{AgentInstance, AgentProvider, AgentStatus};
use crate::error::AmuxError;
use crate::tmux::PaneInfo;

/// Agent provider for Pi Coding Agent instances.
pub struct PiProvider;

/// Check whether `args` indicates a Pi Coding Agent invocation.
///
/// Pi's CLI sets `process.title = "pi"`, and the binary is named `pi`.
/// When run via Node/Bun the first token in args is the runtime, and a
/// subsequent token will be the path to the `pi` entry point (e.g.
/// `/usr/local/bin/pi` or `dist/cli.js` inside the pi package).
///
/// We require that one of the whitespace-separated tokens has a basename
/// of exactly `pi` (i.e. the last `/`-delimited segment equals `"pi"`).
/// A plain `args.contains("pi")` would false-positive on paths like
/// `.../copilot.lua/copilot/...`.
fn args_contain_pi(args: &str) -> bool {
    args.split_whitespace().any(|token| {
        let basename = token.rsplit('/').next().unwrap_or(token);
        basename == "pi"
    })
}

impl AgentProvider for PiProvider {
    fn name(&self) -> &'static str {
        "pi"
    }

    fn discover(
        &self,
        panes: &[PaneInfo],
        process_table: &ProcessTable,
    ) -> Result<Vec<AgentInstance>, AmuxError> {
        let mut instances = Vec::new();

        for pane in panes {
            let is_pi = process_table.has_process_in_tree(pane.pane_pid, &|process_info| {
                // Branch 1: compiled Bun binary — comm is "pi" directly.
                if process_info.comm == "pi" {
                    return true;
                }

                // Branch 2: npm/node-based Pi running under Node.js or Bun.
                (process_info.comm == "node" || process_info.comm == "bun")
                    && args_contain_pi(&process_info.args)
            });

            if !is_pi {
                continue;
            }

            let status = status_file::read_status_file("pi", &pane.pane_id, |s| match s {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_args_contain_pi_direct_binary() {
        assert!(args_contain_pi("/usr/local/bin/pi"));
        assert!(args_contain_pi("/opt/homebrew/bin/pi --interactive"));
    }

    #[test]
    fn test_args_contain_pi_node_invocation() {
        assert!(args_contain_pi(
            "/usr/bin/node /home/user/.npm/bin/pi --workspace /tmp"
        ));
        assert!(args_contain_pi("/usr/bin/bun /usr/local/bin/pi"));
    }

    #[test]
    fn test_args_contain_pi_rejects_substring_matches() {
        // Copilot LSP should not match.
        assert!(!args_contain_pi(
            "node /Users/user/.local/share/nvim/lazy/copilot.lua/copilot/js/language-server.js --stdio"
        ));
        // "spin" should not match.
        assert!(!args_contain_pi("node /usr/bin/spin"));
        // "pip" should not match.
        assert!(!args_contain_pi("/usr/bin/pip install foo"));
        // "api-server" should not match.
        assert!(!args_contain_pi("node /app/api-server.js"));
    }

    #[test]
    fn test_args_contain_pi_empty() {
        assert!(!args_contain_pi(""));
    }
}
