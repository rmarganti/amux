use clap::Parser;

mod agent;
mod cli;
mod error;
mod tmux;

use cli::{Cli, Command};

fn main() {
    let cli = Cli::parse();

    let result = match cli.command {
        Command::List { status, plain } => cli::list::run(status, plain),
        Command::Setup { target } => match target {
            cli::SetupTarget::Amp => cli::setup_amp::run(),
            cli::SetupTarget::Gemini => cli::setup_gemini::run(),
            cli::SetupTarget::Opencode => cli::setup_opencode::run(),
        },
        Command::Status => cli::status::run(),
    };

    if let Err(e) = result {
        eprintln!("{e}");
        std::process::exit(1);
    }
}
