use clap::Parser;

mod agent;
mod cli;
mod error;
mod tmux;

use cli::{Cli, Command};

fn main() {
    let cli = Cli::parse();

    let result = match cli.command {
        Command::List => cli::list::run(),
        Command::Setup { target } => match target {
            cli::SetupTarget::Opencode => cli::setup_opencode::run(),
        },
        Command::Status => cli::status::run(),
    };

    if let Err(e) = result {
        eprintln!("{e}");
        std::process::exit(1);
    }
}
