pub mod amp;
pub mod codex;
pub mod gemini;
pub mod opencode;
pub mod pi;
pub mod process_table;
pub mod status_file;

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
#[derive(Debug, Clone)]
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
        Box::new(amp::AmpProvider),
        Box::new(codex::CodexProvider),
        Box::new(gemini::GeminiProvider),
        Box::new(opencode::OpenCodeProvider),
        Box::new(pi::PiProvider),
    ]
}

/// Attach statuses from the shared per-pane status file and de-duplicate panes.
///
/// Provider discovery is intentionally detection-only. This post-detection pass
/// reads at most one status file per pane. If a pane was detected by multiple
/// providers and the status file names one of them, that provider wins and all
/// non-matching detections are ignored. Missing, stale, or invalid files fall
/// back to the first detection with `Idle` status.
pub fn enrich_detected_statuses(instances: Vec<AgentInstance>) -> Vec<AgentInstance> {
    use std::collections::HashSet;

    let mut output = Vec::new();
    let mut seen_panes = HashSet::new();

    for instance in &instances {
        if !seen_panes.insert(instance.pane.pane_id.clone()) {
            continue;
        }

        let detections: Vec<&AgentInstance> = instances
            .iter()
            .filter(|candidate| candidate.pane.pane_id == instance.pane.pane_id)
            .collect();

        if let Some(status_file) = status_file::read_status_file(&instance.pane.pane_id)
            && let Some(matching) = detections
                .iter()
                .find(|candidate| candidate.provider_name == status_file.provider.as_str())
            && let Some(status) = status_file::normalized_status(&status_file.status)
        {
            let mut enriched = (*matching).clone();
            enriched.status = status;
            output.push(enriched);
            continue;
        }

        let mut fallback = instance.clone();
        fallback.status = AgentStatus::Idle;
        output.push(fallback);
    }

    output
}
