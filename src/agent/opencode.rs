use crate::agent::{AgentInstance, AgentProvider};
use crate::error::AmuxError;
use crate::tmux::PaneInfo;

/// Agent provider for OpenCode instances.
pub struct OpenCodeProvider;

impl AgentProvider for OpenCodeProvider {
    fn name(&self) -> &'static str {
        "opencode"
    }

    fn discover(&self, _panes: &[PaneInfo]) -> Result<Vec<AgentInstance>, AmuxError> {
        // TODO: Walk pane process trees for `opencode` processes,
        //       discover ports via lsof, query status endpoints.
        Ok(vec![])
    }
}
