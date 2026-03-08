use crate::error::AmuxError;

/// Information about a single tmux pane.
#[derive(Debug, Clone)]
pub struct PaneInfo {
    pub session_name: String,
    pub window_name: String,
    pub pane_id: String,
    pub pane_pid: u32,
}

/// Check whether the current process is running inside a tmux session.
pub fn is_inside_tmux() -> bool {
    std::env::var("TMUX").is_ok()
}

/// List all panes across all tmux sessions and windows.
pub fn list_panes() -> Result<Vec<PaneInfo>, AmuxError> {
    // TODO: Run `tmux list-panes -a -F '#{session_name}\t#{window_name}\t#{pane_id}\t#{pane_pid}'`
    //       and parse the output.
    Ok(vec![])
}

/// Switch to the tmux pane identified by session, window, and pane ID.
pub fn switch_to_pane(pane: &PaneInfo) -> Result<(), AmuxError> {
    // TODO: Run tmux switch-client + select-window + select-pane.
    let _ = pane;
    Ok(())
}
