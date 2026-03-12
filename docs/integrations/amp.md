# Amp Integration

amux monitors Amp agent instances running in tmux panes using a
lightweight plugin that runs inside the Amp process.

## Architecture

```
┌──────────────────────┐         ┌──────────────────────────────────────┐
│  Amp process         │         │  amux                                │
│                      │         │                                      │
│  amux-status plugin  │──write──▶  ~/.local/state/amux/amp/            │
│  (hooks into events) │         │  <pane_id>.json                      │
└──────────────────────┘         │                                      │
                                 │  discover:                           │
                                 │    1. Walk tmux pane process trees   │
                                 │    2. Read status file for each pane │
                                 └──────────────────────────────────────┘
```

### Plugin

The `amux-status.ts` plugin runs inside Amp and listens for agent lifecycle
and tool events. On each state transition it writes a JSON status file to
`$XDG_STATE_HOME/amux/amp/<pane_id>.json` (defaulting to
`~/.local/state/amux/amp/`).

Status file format:

```json
{ "status": "idle", "pid": 12345, "ts": 1710000000 }
```

Possible `status` values: `idle`, `busy`, `errored`.

### Cleanup

The plugin registers `process.on('exit')`, `SIGINT`, and `SIGTERM` handlers
that remove the status file when Amp exits. This prevents stale files
from accumulating after normal shutdowns.

For cases where the process is killed without triggering exit handlers (e.g.,
`kill -9`), amux also performs a periodic purge of status files whose recorded
PID is no longer alive (see the main README for details).

### Discovery

1. For each tmux pane, walk the process tree from `pane_pid` to find a child process named `amp`.
2. Read the corresponding status file at `~/.local/state/amux/amp/<pane_id>.json`.
3. If the file is missing or stale (timestamp older than 30 s with no matching live PID), fall back to `idle`.

### Status Mapping

| amux Status        | Plugin Signal                                   |
| ------------------ | ----------------------------------------------- |
| **Running**        | `status: "busy"` (`agent.start`)                |
| **Idle**           | `status: "idle"` or file missing                |
| **Errored**        | `status: "errored"` (`agent.end` with error)    |

## Setup

Install the plugin with:

```sh
amux setup amp
```

This copies `amux-status.ts` to `~/.config/amp/plugins/`, which Amp
discovers on startup when run with `PLUGINS=all amp`. The command is
idempotent — it only overwrites the file when the plugin version has changed.

**Important:** Amp must be launched with `PLUGINS=all` for plugins to be
active:

```sh
PLUGINS=all amp
```

## Process Identification

Amp is installed via `curl -fsSL https://ampcode.com/install.sh | bash` and
runs as a native binary. The process name is `amp`.

**Detection:** Scan tmux pane process trees for a process whose name is
`amp`.
