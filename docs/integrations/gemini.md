# Gemini CLI Integration

amux monitors Gemini CLI agent instances running in tmux panes using a Gemini CLI extension with command hooks that write status files.

## Architecture

```
┌──────────────────────────┐         ┌──────────────────────────────────────┐
│  Gemini CLI process      │         │  amux                                │
│                          │         │                                      │
│  amux-status extension:  │         │  ~/.local/state/amux/                │
│    BeforeAgent ──────────│──write──▶  <pane_id>.json                      │
│    AfterAgent  ──────────│──write──▶                                      │
│    Notification ─────────│──write──▶  discover:                           │
│    SessionEnd  ──────────│──clean──▶    1. Walk tmux pane process trees   │
│                          │         │    2. Read status file for each pane │
└──────────────────────────┘         └──────────────────────────────────────┘
```

### Extension

The `amux-status` extension is installed at `~/.gemini/extensions/amux-status/`
and hooks into Gemini CLI lifecycle events. On each state transition it writes
a JSON status file to `$XDG_STATE_HOME/amux/<pane_id>.json` (defaulting to
`~/.local/state/amux/`).

Status file format:

```json
{ "provider": "gemini", "status": "idle", "pid": 12345, "ts": 1710000000 }
```

Possible `status` values: `idle`, `running`, `awaiting_input`.

The `provider` field identifies which detected agent owns the pane. If more
than one provider is detected in the same pane, amux trusts this field and
ignores non-matching detections.

### Discovery

1. For each tmux pane, walk the process tree from `pane_pid` to find a Gemini CLI process (either a `gemini` binary or `node` running a `gemini` script).
2. Read the corresponding status file at `~/.local/state/amux/<pane_id>.json`.
3. If the file is missing or stale (timestamp older than 30 s with no matching live PID), fall back to `idle`.

### Status Mapping

| amux Status        | Hook Event                                          |
| ------------------ | --------------------------------------------------- |
| **Running**        | `BeforeAgent` (agent starts processing a prompt)    |
| **Idle**           | `AfterAgent` (agent finishes processing) or default |
| **Awaiting Input** | `Notification` with `ToolPermission` type           |

## Setup

Install the extension with:

```sh
amux setup gemini
```

This copies the extension files to `~/.gemini/extensions/amux-status/`. The command is idempotent — it only overwrites files when the extension version has changed.

The extension can also be managed with Gemini CLI's built-in commands:

```sh
gemini extensions disable amux-status
gemini extensions enable amux-status
```

## Process Identification

Gemini CLI can run as:

1. **SEA binary**: A standalone `gemini` executable (Node.js Single Executable Application). Process name: `gemini`.
2. **npm/npx**: `node` running `gemini.js` or similar. Process name is `node` but the command line contains `gemini`.

**Detection:** Scan tmux pane process trees for a process named `gemini`, or a `node` process whose command line arguments contain `gemini`.
