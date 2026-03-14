use std;
use std::collections::HashMap;

/// Information about a single process.
#[derive(Debug, Clone)]
pub struct ProcessInfo {
    /// Process ID.
    pub pid: u32,

    /// Parent process ID.
    pub ppid: u32,

    /// Basename of the executable. Derived from the `comm` column; falls back to the
    /// basename of the first token in `args` when macOS MAXCOMLEN truncation would
    /// yield a mangled path fragment.
    pub comm: String,

    /// Full command-line arguments.
    pub args: String,
}

/// A snapshot of the system process table, indexed for fast tree lookups.
#[derive(Debug)]
pub struct ProcessTable {
    /// Maps PID → process info.
    processes: HashMap<u32, ProcessInfo>,
    /// Maps parent PID → list of child PIDs.
    children: HashMap<u32, Vec<u32>>,
}

impl ProcessTable {
    /// Take a snapshot of the current process table by running `ps` once.
    ///
    /// Returns an empty table if `ps` fails (graceful degradation).
    pub fn snapshot() -> ProcessTable {
        let output = std::process::Command::new("ps")
            .args(["-eo", "pid,ppid,comm,args"])
            .output()
            .ok();

        let Some(output) = output else {
            return ProcessTable::empty();
        };

        if !output.status.success() {
            return ProcessTable::empty();
        }

        let text = String::from_utf8_lossy(&output.stdout);
        Self::parse(&text)
    }

    /// Parse raw `ps -eo pid,ppid,comm,args` output into a `ProcessTable`.
    fn parse(text: &str) -> ProcessTable {
        let mut processes = HashMap::new();
        let mut children: HashMap<u32, Vec<u32>> = HashMap::new();

        for line in text.lines().skip(1) {
            let trimmed = line.trim_start();
            if trimmed.is_empty() {
                continue;
            }

            let Some(info) = Self::parse_line(trimmed) else {
                continue;
            };

            children.entry(info.ppid).or_default().push(info.pid);
            processes.insert(info.pid, info);
        }

        ProcessTable {
            processes,
            children,
        }
    }

    /// Parse a single line of `ps` output into a `ProcessInfo`.
    ///
    /// Expected format: `<pid> <ppid> <comm> <args...>`
    fn parse_line(line: &str) -> Option<ProcessInfo> {
        // Split into whitespace-separated tokens, preserving the rest for args.
        let mut iter = line.splitn(2, char::is_whitespace);
        let pid = iter.next()?.trim().parse::<u32>().ok()?;
        let rest = iter.next()?.trim_start();

        let mut iter = rest.splitn(2, char::is_whitespace);
        let ppid = iter.next()?.trim().parse::<u32>().ok()?;
        let rest = iter.next()?.trim_start();

        // comm is the next token; args is everything after.
        let (comm, args) = match rest.splitn(2, char::is_whitespace).collect::<Vec<_>>()[..] {
            [c, a] => (c, a.trim_start()),
            [c] => (c, ""),
            _ => return None,
        };

        // `comm` from ps may be a full path; take the basename.
        let raw_basename = comm.rsplit('/').next().unwrap_or(comm);

        // On macOS, MAXCOMLEN (15 chars) causes the kernel to truncate long absolute paths
        // stored in `p_comm`. When the original `comm` value contains a '/', it indicates
        // a path (even if truncated). In such cases, fall back to deriving the basename from
        // the first token of `args`, which is never truncated by the kernel.
        let comm = if comm.contains('/') {
            args.split_whitespace()
                .next()
                .and_then(|p| p.rsplit('/').next())
                .unwrap_or(raw_basename)
        } else {
            raw_basename
        };

        Some(ProcessInfo {
            pid,
            ppid,
            comm: comm.to_string(),
            args: args.to_string(),
        })
    }

    /// Check whether any process in the subtree rooted at `root_pid` matches
    /// the predicate. Uses iterative DFS to avoid stack overflow.
    pub fn has_process_in_tree(
        &self,
        root_pid: u32,
        matcher: &dyn Fn(&ProcessInfo) -> bool,
    ) -> bool {
        let mut stack = vec![root_pid];

        while let Some(pid) = stack.pop() {
            if let Some(info) = self.processes.get(&pid)
                && matcher(info)
            {
                return true;
            }

            if let Some(kids) = self.children.get(&pid) {
                stack.extend(kids);
            }
        }

        false
    }

    /// Create an empty process table.
    fn empty() -> ProcessTable {
        ProcessTable {
            processes: HashMap::new(),
            children: HashMap::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE_PS_OUTPUT: &str = "\
  PID  PPID COMM             ARGS
    1     0 launchd          /sbin/launchd
  100     1 bash             /bin/bash --login
  200   100 node             /usr/bin/node /home/user/.npm/bin/gemini
  300   100 opencode         opencode --workspace /tmp
  400   200 cat              cat /dev/null
";

    const HOMEBREW_PS_OUTPUT: &str = "\
  PID  PPID COMM             ARGS
    1     0 launchd          /sbin/launchd
  100     1 bash             /bin/bash --login
  200   100 /opt/homebrew/op /opt/homebrew/opt/node/bin/node /opt/homebrew/bin/gemini
  201   100 /opt/homebrew/Ce /opt/homebrew/Cellar/node/25.6.1_1/bin/node /opt/homebrew/bin/gemini
";

    #[test]
    fn test_parse_ps_output() {
        let table = ProcessTable::parse(SAMPLE_PS_OUTPUT);

        assert_eq!(table.processes.len(), 5);

        let p1 = &table.processes[&1];
        assert_eq!(p1.pid, 1);
        assert_eq!(p1.ppid, 0);
        assert_eq!(p1.comm, "launchd");
        assert_eq!(p1.args, "/sbin/launchd");

        let p200 = &table.processes[&200];
        assert_eq!(p200.pid, 200);
        assert_eq!(p200.ppid, 100);
        assert_eq!(p200.comm, "node");
        assert_eq!(p200.args, "/usr/bin/node /home/user/.npm/bin/gemini");

        // Check children map.
        let kids_of_100 = &table.children[&100];
        assert!(kids_of_100.contains(&200));
        assert!(kids_of_100.contains(&300));

        let kids_of_200 = &table.children[&200];
        assert!(kids_of_200.contains(&400));
    }

    #[test]
    fn test_tree_walk_finds_nested_process() {
        let table = ProcessTable::parse(SAMPLE_PS_OUTPUT);

        // `cat` (pid 400) is a grandchild of pid 100.
        let found = table.has_process_in_tree(100, &|info| info.comm == "cat");
        assert!(found);
    }

    #[test]
    fn test_tree_walk_returns_false_when_no_match() {
        let table = ProcessTable::parse(SAMPLE_PS_OUTPUT);

        let found = table.has_process_in_tree(100, &|info| info.comm == "vim");
        assert!(!found);
    }

    #[test]
    fn test_parse_truncated_comm_derives_basename_from_args() {
        let table = ProcessTable::parse(HOMEBREW_PS_OUTPUT);

        // Both processes must have comm == "node", not "op" or "Ce".
        let p200 = &table.processes[&200];
        assert_eq!(p200.comm, "node");
        assert_eq!(
            p200.args,
            "/opt/homebrew/opt/node/bin/node /opt/homebrew/bin/gemini"
        );

        let p201 = &table.processes[&201];
        assert_eq!(p201.comm, "node");
        assert_eq!(
            p201.args,
            "/opt/homebrew/Cellar/node/25.6.1_1/bin/node /opt/homebrew/bin/gemini"
        );
    }

    #[test]
    fn test_homebrew_node_gemini_is_detected_in_tree() {
        let table = ProcessTable::parse(HOMEBREW_PS_OUTPUT);

        // Simulate the Gemini detection predicate used in gemini.rs.
        let found = table.has_process_in_tree(100, &|info| {
            info.comm == "gemini" || (info.comm == "node" && info.args.contains("gemini"))
        });
        assert!(
            found,
            "Gemini should be detected under a Homebrew Node process"
        );
    }
}
