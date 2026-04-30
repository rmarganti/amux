# amux

A CLI utility for monitoring AI coding agents running in tmux panes. Scans for known agent processes, reports their status, and provides quick navigation. Observational only — it does not launch, restart, or interact with agents.

## Supported Agents

- **Amp**
- **OpenCode**
- **Gemini CLI**
- **Pi Coding Agent**

## Commands

### `amux list`

Scans all tmux panes for agents, pipes results through `fzf`, and switches to the selected pane. In the fzf picker, use keyboard shortcuts to filter by status:

| Shortcut | Filter         |
| -------- | -------------- |
| `ctrl-a` | All            |
| `ctrl-r` | Running        |
| `ctrl-i` | Idle           |
| `ctrl-w` | Awaiting input |
| `ctrl-e` | Errored        |

Options:

- `--status <STATUS>` — filter agents by status (`running`, `idle`, `awaiting-input`, `errored`)
- `--plain` — output plain text to stdout instead of launching fzf (for scripting and fzf reload)

```
[session > window] agent - status
```

### `amux status`

Outputs a terse summary with tmux format strings for statusline use, including each agent's tmux session name.

```
#[fg=green]●#[default] work  #[fg=yellow]⚠#[default] main
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

### Amp

Install the status plugin so amux can monitor Amp instances:

```sh
amux setup amp
```

This copies a lightweight plugin to `~/.config/amp/plugins/`. Amp must be launched with `PLUGINS=all` for plugins to be active:

```sh
PLUGINS=all amp
```

Restart any running Amp instances after installing. See [docs/integrations/amp.md](docs/integrations/amp.md) for details.

### OpenCode

Install the status plugin so amux can monitor OpenCode instances:

```sh
amux setup opencode
```

This copies a lightweight plugin to `~/.config/opencode/plugins/` that reports agent status via the filesystem. Restart any running OpenCode instances after installing. See [docs/integrations/opencode.md](docs/integrations/opencode.md) for details.

### Gemini CLI

Install the status extension so amux can monitor Gemini CLI instances:

```sh
amux setup gemini
```

This copies an extension to `~/.gemini/extensions/amux-status/` that reports agent status via the filesystem. Restart any running Gemini CLI instances after installing. See [docs/integrations/gemini.md](docs/integrations/gemini.md) for details.

### Pi Coding Agent

Install the status extension so amux can monitor Pi instances:

```sh
amux setup pi
```

This copies an extension to `~/.pi/agent/extensions/amux-status/` that reports agent status via the filesystem. Restart any running Pi instances after installing. See [docs/integrations/pi.md](docs/integrations/pi.md) for details.

## Tmux Integration

Add to your `tmux.conf`:

```tmux
# Statusline — show agent summary
set -g status-right '#(amux status)'

# Popup — browse and jump to agents with <prefix> + a
bind a display-popup -E -w 80% -h 70% "amux list"
```

Reload with `tmux source-file ~/.tmux.conf`.
