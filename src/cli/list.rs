use crate::error::AmuxError;

pub fn run() -> Result<(), AmuxError> {
    // TODO: Scan panes, discover agents, pipe through fzf, navigate to selection.
    if !crate::tmux::is_inside_tmux() {
        return Err(AmuxError::NotInTmux);
    }

    Ok(())
}
