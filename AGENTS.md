# amux

A CLI utility for monitoring AI coding agents running in tmux panes. Scans for
known agent processes, reports their status, and provides quick navigation.
Observational only — it does not launch, restart, or interact with agents.

## Repo organization

Root
├─ /docs: dev/design docs, not meant for end user (should be kept up-to-date)
├─┬ /plugin: Agent specific plugins
│ ├─ /plugin/amp: Amp Code status extension
│ ├─ /plugin/gemini: Gemini CLI status extension
│ └─ /plugin/opencode: OpenCode status plugin
├─ /src/agent: agent-specific implementation
├─ /src/cli: command line definition and implementation
└─ /src/tmux: module for interacting with Tmux (interacting with sessions, windows, panes)

## Verifying (MUST BE RUN BEFORE CONSIDERING A TASK COMPLETE)

- `cargo fmt --all -- --check`
- `cargo test --all-features`
- `cargo clippy --all-targets --all-features -- -D warnings`
- `cargo build --all-features`
