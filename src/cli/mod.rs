pub mod list;
pub mod setup_amp;
pub mod setup_gemini;
pub mod setup_opencode;
pub mod status;

use clap::{Parser, Subcommand};

use crate::agent::AgentStatus;

#[derive(Parser)]
#[command(name = "amux", about = "Manage and monitor AI coding agents in tmux")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand)]
pub enum Command {
    /// Scan tmux panes for agents and select one via fzf
    List {
        /// Filter agents by status
        #[arg(long)]
        status: Option<AgentStatus>,

        /// Output plain text to stdout instead of launching fzf.
        /// Intended for use with fzf's reload action.
        #[arg(long)]
        plain: bool,
    },
    /// Install agent plugins and configure integrations
    Setup {
        #[command(subcommand)]
        target: SetupTarget,
    },
    /// Output a terse status string for tmux statusline interpolation
    Status,
}

#[derive(Subcommand)]
pub enum SetupTarget {
    /// Install the amux status plugin for Amp
    Amp,
    /// Install the amux status extension for Gemini CLI
    Gemini,
    /// Install the amux status plugin for OpenCode
    Opencode,
}
