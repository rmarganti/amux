# OpenCode Integration

amux monitors OpenCode agent instances running in tmux panes using a
lightweight plugin that runs inside the OpenCode process.

## Architecture

```
┌──────────────────────┐         ┌──────────────────────────────────────┐
│  OpenCode process    │         │  amux                                │
│                      │         │                                      │
│  amux-status plugin  │──write──▶  ~/.local/state/amux/opencode/       │
│  (hooks into events) │         │  <pane_id>.json                      │
└──────────────────────┘         │                                      │
                                 │  discover:                           │
                                 │    1. Walk tmux pane process trees   │
                                 │    2. Read status file for each pane │
                                 └──────────────────────────────────────┘
```

### Plugin

The `amux-status.js` plugin runs inside OpenCode and listens for session and permission events.
On each state transition it writes a JSON status file to `$XDG_STATE_HOME/amux/opencode/<pane_id>.json`
(defaulting to `~/.local/state/amux/opencode/`).

Status file format:

```json
{ "status": "idle", "pid": 12345, "ts": 1710000000 }
```

Possible `status` values: `idle`, `busy`, `awaiting_input`, `errored`.

Internally, OpenCode may emit `session.status` with `status.type: "retry"` while a
session is still actively working. The amux plugin normalizes that to `busy` so
these panes remain shown as running.

### Cleanup

The plugin registers `process.on('exit')`, `SIGINT`, and `SIGTERM` handlers
that remove the status file when OpenCode exits. This prevents stale files
from accumulating after normal shutdowns.

For cases where the process is killed without triggering exit handlers (e.g.,
`kill -9`), amux also performs a periodic purge of status files whose recorded
PID is no longer alive (see the main README for details).

### Discovery

1. For each tmux pane, walk the process tree from `pane_pid` to find a child process named `opencode`.
2. Read the corresponding status file at `~/.local/state/amux/opencode/<pane_id>.json`.
3. If the file is missing or stale (timestamp older than 30 s with no matching live PID), fall back to `idle`.

### Status Mapping

| amux Status        | Plugin Signal                    |
| ------------------ | -------------------------------- |
| **Running**        | `status: "busy"` (including OpenCode `retry`) |
| **Idle**           | `status: "idle"` or file missing |
| **Awaiting Input** | `status: "awaiting_input"`       |
| **Errored**        | `status: "errored"`              |

## Setup

Install the plugin with:

```sh
amux setup opencode
```

This copies `amux-status.js` to `~/.config/opencode/plugins/`, which OpenCode
auto-discovers on startup. The command is idempotent — it only overwrites the
file when the plugin version has changed. It also removes the legacy
`~/.config/opencode/plugin/amux-status.js` path if present.

## Process Identification

When installed via Homebrew, OpenCode is a native binary at
`/opt/homebrew/bin/opencode`. When installed via npm/bun, a wrapper script
spawns a platform-specific binary (e.g., `opencode-darwin-arm64`).

**Detection:** Scan tmux pane process trees for a process whose name starts
with `opencode`. Tmux exposes `#{pane_pid}` per pane; amux walks child
processes (via `pgrep -P`) to find the actual `opencode` process.
