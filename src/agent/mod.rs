pub mod opencode;

use crate::error::AmuxError;
use crate::tmux::PaneInfo;

/// The observed status of an agent instance.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AgentStatus {
    /// Agent is actively processing (LLM call, tool execution).
    Running,
    /// Agent is running but not actively processing.
    Idle,
    /// Agent is waiting for user interaction (e.g., permission approval).
    AwaitingInput,
    /// Agent has encountered an error and is retrying.
    Errored,
}

/// A discovered agent instance tied to a specific tmux pane.
#[derive(Debug)]
pub struct AgentInstance {
    pub pane: PaneInfo,
    pub provider_name: &'static str,
    pub status: AgentStatus,
}

/// Trait implemented by each agent type to provide discovery and status reading.
pub trait AgentProvider {
    /// A human-readable name for this agent type.
    fn name(&self) -> &'static str;

    /// Scan the given tmux panes and return discovered agent instances.
    fn discover(&self, panes: &[PaneInfo]) -> Result<Vec<AgentInstance>, AmuxError>;
}
