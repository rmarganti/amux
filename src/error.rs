use thiserror::Error;

#[derive(Debug, Error)]
pub enum AmuxError {
    #[error("not running inside a tmux session")]
    NotInTmux,

    #[error("no agents found")]
    NoAgentsFound,

    #[error("tmux error: {0}")]
    Tmux(String),

    #[error("agent provider error: {0}")]
    Provider(String),

    #[error("fzf exited without a selection")]
    FzfNoSelection,

    #[error("fzf error: {0}")]
    Fzf(String),
}
