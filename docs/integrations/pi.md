# Pi Coding Agent Integration

amux monitors Pi Coding Agent instances running in tmux panes using a
lightweight extension that runs inside the Pi process.

## Architecture

```
┌──────────────────────┐         ┌──────────────────────────────────────┐
│  Pi process          │         │  amux                                │
│                      │         │                                      │
│  amux-status ext     │──write──▶  ~/.local/state/amux/pi/             │
│  (hooks into events) │         │  <pane_id>.json                      │
└──────────────────────┘         │                                      │
                                 │  discover:                           │
                                 │    1. Walk tmux pane process trees   │
                                 │    2. Read status file for each pane │
                                 └──────────────────────────────────────┘
```

### Extension

The `amux-status.ts` extension runs inside Pi and listens for agent lifecycle
events. On each state transition it writes a JSON status file to
`$XDG_STATE_HOME/amux/pi/<pane_id>.json` (defaulting to
`~/.local/state/amux/pi/`).

Status file format:

```json
{ "status": "idle", "pid": 12345, "ts": 1710000000 }
```

Possible `status` values: `idle`, `busy`, `awaiting_input`, `errored`.

### Cleanup

The extension registers `process.on('exit')`, `SIGINT`, and `SIGTERM` handlers
that remove the status file when Pi exits. It also listens for `session_shutdown`
to clean up. This prevents stale files from accumulating after normal shutdowns.

For cases where the process is killed without triggering exit handlers (e.g.,
`kill -9`), amux also performs a periodic purge of status files whose recorded
PID is no longer alive (see the main README for details).

### Discovery

1. For each tmux pane, walk the process tree from `pane_pid` to find a child process named `pi`.
2. Read the corresponding status file at `~/.local/state/amux/pi/<pane_id>.json`.
3. If the file is missing or stale (timestamp older than 30 s with no matching live PID), fall back to `idle`.

### Status Mapping

| amux Status        | Extension Signal                                 |
| ------------------ | ------------------------------------------------ |
| **Running**        | `status: "busy"` (`agent_start`)                 |
| **Idle**           | `status: "idle"` (`agent_end`) or file missing   |
| **Awaiting Input** | `status: "awaiting_input"`                        |
| **Errored**        | `status: "errored"`                               |

## Setup

Install the extension with:

```sh
amux setup pi
```

This copies `amux-status.ts` to `~/.pi/agent/extensions/amux-status/index.ts`,
which Pi discovers on startup. The command is idempotent — it only overwrites
the file when the extension version has changed.

## Process Identification

Pi can run as a compiled Bun binary (`pi`) or via Node.js/Bun as a package.
The CLI entry point sets `process.title = "pi"`.

**Detection:** Scan tmux pane process trees for a process named `pi`, or a
`node`/`bun` process whose args contain `pi`.
