pub mod gemini;
pub mod opencode;
pub mod process_table;

use clap::ValueEnum;

use crate::error::AmuxError;
use crate::tmux::PaneInfo;

/// The observed status of an agent instance.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum AgentStatus {
    /// Agent is actively processing (LLM call, tool execution).
    Running,

    /// Agent is running but not actively processing.
    Idle,

    /// Agent is waiting for user interaction (e.g., permission approval).
    #[value(name = "awaiting-input")]
    AwaitingInput,

    /// Agent has encountered an error and is retrying.
    Errored,
}

impl std::fmt::Display for AgentStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AgentStatus::Running => write!(f, "running"),
            AgentStatus::Idle => write!(f, "idle"),
            AgentStatus::AwaitingInput => write!(f, "awaiting input"),
            AgentStatus::Errored => write!(f, "errored"),
        }
    }
}

/// A discovered agent instance tied to a specific tmux pane.
#[derive(Debug)]
pub struct AgentInstance {
    pub pane: PaneInfo,
    pub provider_name: &'static str,
    pub status: AgentStatus,
}

/// Trait implemented by each agent type to provide discovery and status reading.
///
/// `Send + Sync` bounds allow providers to be shared across threads when
/// callers use `std::thread::scope` to run discovery in parallel.
pub trait AgentProvider: Send + Sync {
    /// A human-readable name for this agent type.
    fn name(&self) -> &'static str;

    /// Scan the given tmux panes and return discovered agent instances.
    fn discover(
        &self,
        panes: &[PaneInfo],
        process_table: &process_table::ProcessTable,
    ) -> Result<Vec<AgentInstance>, AmuxError>;
}

/// Returns all registered agent providers.
pub fn all_providers() -> Vec<Box<dyn AgentProvider>> {
    vec![
        Box::new(gemini::GeminiProvider),
        Box::new(opencode::OpenCodeProvider),
    ]
}
