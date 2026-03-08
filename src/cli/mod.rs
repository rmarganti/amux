pub mod list;
pub mod status;

use clap::Parser;

#[derive(Parser)]
#[command(name = "amux", about = "Manage and monitor AI coding agents in tmux")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Parser)]
pub enum Command {
    /// Scan tmux panes for agents and select one via fzf
    List,
    /// Output a terse status string for tmux statusline interpolation
    Status,
}
