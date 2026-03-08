use crate::error::AmuxError;

/// Information about a single tmux pane.
#[derive(Debug, Clone, PartialEq, Eq)]
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

/// Abstraction over tmux command execution for testability.
pub trait TmuxRunner {
    /// Run a tmux command with the given arguments and return stdout.
    fn run(&self, args: &[&str]) -> Result<String, AmuxError>;
}

/// Default runner that shells out to the real `tmux` binary.
pub struct SystemTmuxRunner;

impl TmuxRunner for SystemTmuxRunner {
    fn run(&self, args: &[&str]) -> Result<String, AmuxError> {
        let output = std::process::Command::new("tmux")
            .args(args)
            .output()
            .map_err(|e| AmuxError::Tmux(format!("failed to execute tmux: {e}")))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(AmuxError::Tmux(stderr.trim().to_string()));
        }

        Ok(String::from_utf8_lossy(&output.stdout).into_owned())
    }
}

const LIST_PANES_FORMAT: &str = "#{session_name}\t#{window_name}\t#{pane_id}\t#{pane_pid}";

/// List all panes across all tmux sessions and windows.
pub fn list_panes(runner: &dyn TmuxRunner) -> Result<Vec<PaneInfo>, AmuxError> {
    let output = runner.run(&["list-panes", "-a", "-F", LIST_PANES_FORMAT])?;
    parse_list_panes_output(&output)
}

fn parse_list_panes_output(output: &str) -> Result<Vec<PaneInfo>, AmuxError> {
    let mut panes = Vec::new();

    for line in output.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        let parts: Vec<&str> = line.split('\t').collect();
        if parts.len() != 4 {
            return Err(AmuxError::Tmux(format!(
                "unexpected list-panes output: {line}"
            )));
        }

        let pane_pid: u32 = parts[3]
            .parse()
            .map_err(|_| AmuxError::Tmux(format!("invalid pane pid: {}", parts[3])))?;

        panes.push(PaneInfo {
            session_name: parts[0].to_string(),
            window_name: parts[1].to_string(),
            pane_id: parts[2].to_string(),
            pane_pid,
        });
    }

    Ok(panes)
}

/// Switch to the tmux pane identified by session, window, and pane ID.
pub fn switch_to_pane(runner: &dyn TmuxRunner, pane: &PaneInfo) -> Result<(), AmuxError> {
    let target_window = format!("{}:{}", pane.session_name, pane.window_name);
    runner.run(&["switch-client", "-t", &pane.session_name])?;
    runner.run(&["select-window", "-t", &target_window])?;
    runner.run(&["select-pane", "-t", &pane.pane_id])?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_list_panes_output_single_pane() {
        let output = "main\teditor\t%0\t1234\n";
        let panes = parse_list_panes_output(output).unwrap();
        assert_eq!(panes.len(), 1);
        assert_eq!(
            panes[0],
            PaneInfo {
                session_name: "main".to_string(),
                window_name: "editor".to_string(),
                pane_id: "%0".to_string(),
                pane_pid: 1234,
            }
        );
    }

    #[test]
    fn test_parse_list_panes_output_multiple_panes() {
        let output = "main\teditor\t%0\t1234\nwork\tshell\t%1\t5678\nwork\tshell\t%2\t9012\n";
        let panes = parse_list_panes_output(output).unwrap();
        assert_eq!(panes.len(), 3);
        assert_eq!(panes[0].session_name, "main");
        assert_eq!(panes[1].session_name, "work");
        assert_eq!(panes[2].pane_id, "%2");
    }

    #[test]
    fn test_parse_list_panes_output_empty() {
        let output = "";
        let panes = parse_list_panes_output(output).unwrap();
        assert!(panes.is_empty());
    }

    #[test]
    fn test_parse_list_panes_output_blank_lines() {
        let output = "\nmain\teditor\t%0\t1234\n\n";
        let panes = parse_list_panes_output(output).unwrap();
        assert_eq!(panes.len(), 1);
    }

    #[test]
    fn test_parse_list_panes_output_bad_columns() {
        let output = "main\teditor\t%0\n";
        let result = parse_list_panes_output(output);
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_list_panes_output_bad_pid() {
        let output = "main\teditor\t%0\tnotanumber\n";
        let result = parse_list_panes_output(output);
        assert!(result.is_err());
    }

    /// A mock TmuxRunner that records calls and returns predefined responses.
    struct MockTmuxRunner {
        responses: std::cell::RefCell<Vec<Result<String, AmuxError>>>,
        calls: std::cell::RefCell<Vec<Vec<String>>>,
    }

    impl MockTmuxRunner {
        fn new(responses: Vec<Result<String, AmuxError>>) -> Self {
            Self {
                responses: std::cell::RefCell::new(responses.into_iter().rev().collect()),
                calls: std::cell::RefCell::new(Vec::new()),
            }
        }

        fn calls(&self) -> Vec<Vec<String>> {
            self.calls.borrow().clone()
        }
    }

    impl TmuxRunner for MockTmuxRunner {
        fn run(&self, args: &[&str]) -> Result<String, AmuxError> {
            self.calls
                .borrow_mut()
                .push(args.iter().map(|s| s.to_string()).collect());
            self.responses
                .borrow_mut()
                .pop()
                .unwrap_or(Err(AmuxError::Tmux("no mock response".to_string())))
        }
    }

    #[test]
    fn test_list_panes_delegates_to_runner() {
        let mock = MockTmuxRunner::new(vec![Ok("sess\twin\t%0\t42\n".to_string())]);
        let panes = list_panes(&mock).unwrap();
        assert_eq!(panes.len(), 1);
        assert_eq!(panes[0].pane_pid, 42);

        let calls = mock.calls();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0], vec!["list-panes", "-a", "-F", LIST_PANES_FORMAT]);
    }

    #[test]
    fn test_list_panes_propagates_error() {
        let mock = MockTmuxRunner::new(vec![Err(AmuxError::Tmux("server exited".to_string()))]);
        let result = list_panes(&mock);
        assert!(result.is_err());
    }

    #[test]
    fn test_switch_to_pane_runs_correct_commands() {
        let mock = MockTmuxRunner::new(vec![
            Ok(String::new()),
            Ok(String::new()),
            Ok(String::new()),
        ]);

        let pane = PaneInfo {
            session_name: "work".to_string(),
            window_name: "code".to_string(),
            pane_id: "%3".to_string(),
            pane_pid: 100,
        };

        switch_to_pane(&mock, &pane).unwrap();

        let calls = mock.calls();
        assert_eq!(calls.len(), 3);
        assert_eq!(calls[0], vec!["switch-client", "-t", "work"]);
        assert_eq!(calls[1], vec!["select-window", "-t", "work:code"]);
        assert_eq!(calls[2], vec!["select-pane", "-t", "%3"]);
    }

    #[test]
    fn test_switch_to_pane_stops_on_first_error() {
        let mock = MockTmuxRunner::new(vec![Err(AmuxError::Tmux("session not found".to_string()))]);

        let pane = PaneInfo {
            session_name: "gone".to_string(),
            window_name: "x".to_string(),
            pane_id: "%0".to_string(),
            pane_pid: 1,
        };

        let result = switch_to_pane(&mock, &pane);
        assert!(result.is_err());
        assert_eq!(mock.calls().len(), 1);
    }
}
