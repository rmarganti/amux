# Codex Integration

amux monitors Codex CLI instances running in tmux panes using Codex's installed
hook mechanism. No wrapper is required.

## Architecture

```
┌──────────────────────┐         ┌──────────────────────────────────────┐
│  Codex CLI process   │         │  amux                                │
│                      │         │                                      │
│  Codex hooks         │──write──▶  ~/.local/state/amux/                │
│  amux-status.sh      │         │  <pane_id>.json                      │
└──────────────────────┘         │                                      │
                                 │  discover:                           │
                                 │    1. Walk tmux pane process trees   │
                                 │    2. Read status file for each pane │
                                 └──────────────────────────────────────┘
```

The hook writes status files to `$XDG_STATE_HOME/amux/<pane_id>.json`,
defaulting to `~/.local/state/amux/`.

Status file format:

```json
{
    "provider": "codex",
    "status": "idle",
    "pid": 12345,
    "ts": 1710000000,
    "event": "Stop"
}
```

The `pid` is the live Codex process PID, not the short-lived hook shell PID, so
amux does not purge active Codex status files as stale.

Possible `status` values: `idle`, `running`, `awaiting_input`, `errored`.

The `provider` field identifies which detected agent owns the pane. If more
than one provider is detected in the same pane, amux trusts this field and
ignores non-matching detections.

## Setup

Install the hook with:

```sh
amux setup codex
```

This command:

1. Copies `amux-status.sh` to `~/.codex/hooks/amux-status.sh`.
2. Marks it executable.
3. Merges amux hook entries into `~/.codex/hooks.json` for session, prompt,
   tool, permission, subagent, and stop events.

The merge is conservative: existing user hooks and unrelated settings are
preserved. Existing amux-managed Codex hook entries are replaced so rerunning the
command is idempotent.

## Status Mapping

| amux Status        | Codex Hook Signal                                                 |
| ------------------ | ----------------------------------------------------------------- |
| **Running**        | `UserPromptSubmit`, tool/subagent events, `Start`, `task_started` |
| **Idle**           | `SessionStart`, `Stop`, `agent-turn-complete`                     |
| **Awaiting Input** | `PermissionRequest`, approval/user-input requests                 |
| **Errored**        | `Error`, `error`, `errored`                                       |

Codex can emit `Stop` at the end of an assistant turn, which may happen between
model and tool phases. To avoid marking an active task idle between those phases,
amux also registers tool and subagent hooks (with `matcher: "*"`) that move the
pane back to `running` as soon as work continues. The hook script recognizes
additional event names for compatibility with versions that emit richer lifecycle
events.

## Discovery

1. For each tmux pane, walk the process tree from `pane_pid` to find a child
   process named `codex`, a process whose name starts with `codex-`, or an
   executable path whose basename matches those forms.
2. Read the corresponding status file at `~/.local/state/amux/<pane_id>.json`.
3. If the file is missing or stale, fall back to `idle`.
