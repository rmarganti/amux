# amux

A CLI utility for monitoring AI coding agents running in tmux panes. Scans for known agent processes, reports their status, and provides quick navigation. Observational only — it does not launch, restart, or interact with agents.

## Supported Agents

- **OpenCode**

## Commands

### `amux list`

Scans all tmux panes for agents, pipes results through `fzf`, and switches to the selected pane.

```
[session > window] agent - status
```

### `amux status`

Outputs a terse summary with tmux format strings for statusline use.

```
#[fg=green]●2 #[fg=yellow]⚠1
```

Indicators: `●` running, `○` idle, `⚠` awaiting input, `✖` errored.

## Prerequisites

- Rust toolchain
- `fzf` (for `amux list`)
- tmux

## Install

```sh
cargo install --path .
```

## Agent Setup

### OpenCode

Install the status plugin so amux can monitor OpenCode instances:

```sh
amux setup opencode
```

This copies a lightweight plugin to `~/.config/opencode/plugin/` that reports agent status via the filesystem. Restart any running OpenCode instances after installing. See [docs/integrations/opencode.md](docs/integrations/opencode.md) for details.

## Tmux Integration

Add to your `tmux.conf`:

```tmux
# Statusline — show agent summary
set -g status-right '#(amux status)'

# Popup — browse and jump to agents with <prefix> + a
bind a display-popup -E "amux list"
```

Reload with `tmux source-file ~/.tmux.conf`.
