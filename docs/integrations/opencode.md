# OpenCode Integration

Investigation into how amux can discover and monitor OpenCode agent instances running in tmux panes.

## Process Identification

When installed via Homebrew (the recommended macOS path), OpenCode is a native Mach-O binary at `/opt/homebrew/bin/opencode` (symlinked from the Cellar). When installed via npm/bun, a Node.js wrapper script (`bin/opencode`) spawns a platform-specific native binary (e.g., `opencode-darwin-arm64`).

**Detection strategy:** Scan tmux pane process trees for a process whose name starts with `opencode`. Tmux exposes `#{pane_pid}` per pane; walk child processes (via `pgrep -P` or `/proc` on Linux, `libproc` on macOS) to find the actual `opencode` process. The `pane_current_command` field may show `opencode` directly if it's the foreground process, or `bash`/`zsh` if the shell is the direct child.

**Environment variable:** OpenCode sets `OPENCODE_PID=<pid>` in its own process environment at startup (`src/index.ts`). This is useful for confirmation but not for initial discovery.

## Server & API

OpenCode runs an embedded HTTP server (Hono) alongside its TUI. The server provides a REST API usable for status queries.

### Port Discovery

- **Default port:** 4096 (falls back to a random port if 4096 is taken).
- **Explicit port:** `--port` CLI flag or `server.port` in `~/.config/opencode/opencode.json`.
- **No port/URL file on disk.** There is no lockfile or state file written to `~/.local/state/opencode` that records the running server's port.
- **mDNS:** Optional; disabled by default. When enabled, publishes as `opencode-<port>.local` via Bonjour.

**Discovery strategy for amux:**

1. **Try default port first.** Attempt `GET http://127.0.0.1:4096/global/health`. If the response is `{ "healthy": true, "version": "..." }`, we've found it.
2. **Scan open ports of the opencode process.** Once we know the PID (from tmux pane process tree), use `lsof -iTCP -sTCP:LISTEN -a -p <pid>` to find which port the process is listening on.
3. **Fall back to config.** Read `~/.config/opencode/opencode.json` → `server.port` if set.

Option 2 is the most reliable for arbitrary setups. Option 1 is the fast path for the common case.

### Key Endpoints

All project-scoped endpoints require a `directory` query parameter or `x-opencode-directory` header to scope to the correct project instance.

| Endpoint | Method | Description |
|---|---|---|
| `/global/health` | GET | Health check — `{ healthy: true, version: "..." }` |
| `/session/status` | GET | Returns `Record<sessionID, SessionStatus>` for all active sessions |
| `/permission` | GET | Lists all pending permission requests (awaiting user input) |
| `/event` | GET | SSE stream of all bus events (including `session.status`, `permission.asked`) |

## Session Status Model

OpenCode defines three session statuses (`src/session/status.ts`):

| Status | Meaning |
|---|---|
| `idle` | No active work. (Sessions in this state are **absent** from the status map — not present as `"idle"`.) |
| `busy` | Agent is actively processing (LLM call, tool execution). |
| `retry` | A transient error occurred; the agent is waiting before retrying. Includes `attempt`, `message`, and `next` (timestamp) fields. |

**There is no explicit "awaiting input" status.** When the agent needs user approval for a tool call, the session remains `busy` while the `PermissionNext.ask()` promise blocks. The pending permission is surfaced separately via:

- `GET /permission` — returns an array of pending `PermissionNext.Request` objects.
- SSE event `permission.asked` — fired when a permission request is created.
- SSE event `permission.replied` — fired when the user responds.

### Mapping to Amux Status Model

| Amux Status | OpenCode Signal |
|---|---|
| **Running** | Session status is `busy` AND no pending permissions for that session |
| **Idle** | Session is absent from the status map (i.e., `idle`) |
| **Awaiting Input** | Session status is `busy` AND there is a pending permission for that session |
| **Errored** | Session status is `retry` |
| **Completed** | Not directly exposed. Could be inferred from a session transitioning `busy` → `idle`, but there's no persistent "completed" flag. |

## SSE Event Stream

`GET /event` provides a real-time Server-Sent Events stream. Each event is a JSON object:

```json
{ "type": "session.status", "properties": { "sessionID": "...", "status": { "type": "busy" } } }
```

Key events for amux:

- `session.status` — status transitions (`busy`, `idle`, `retry`)
- `permission.asked` — a permission request was created (agent is now awaiting input)
- `permission.replied` — user responded to a permission request

The stream also emits `server.connected` on connect and `server.heartbeat` every 10 seconds.

**Note:** The SSE stream is scoped to a single project directory (determined by the `directory` query param or header). For multi-project monitoring, amux would need one SSE connection per project, or use the global event stream at `GET /global/event` which wraps events with a `directory` field.

## Data Directories

| Path | Purpose |
|---|---|
| `~/.local/share/opencode/opencode.db` | SQLite database (sessions, messages, parts) |
| `~/.local/state/opencode/` | Runtime state (frecency, model selection, prompt history) |
| `~/.config/opencode/opencode.json` | User configuration |
| `~/.cache/opencode/` | Cache (versioned, auto-invalidated on upgrade) |

## Recommended Approach for Amux

### Discovery (AgentProvider::discover)

1. For each tmux pane, get `pane_pid`.
2. Walk the process tree from `pane_pid` to find a child process named `opencode` (or matching `opencode-*` for npm installs).
3. Record the pane location (session, window, pane ID) and the opencode PID.

### Status (AgentProvider::status)

**Polling approach (MVP):**

1. Determine the server port: `lsof -iTCP -sTCP:LISTEN -a -p <pid>` to get the listening port from the discovered PID.
2. Query `GET http://127.0.0.1:<port>/session/status` to get session statuses.
3. Query `GET http://127.0.0.1:<port>/permission` to get pending permissions.
4. Combine: if any session is `busy` with pending permissions → Awaiting Input; if any session is `busy` → Running; if any session is `retry` → Errored; otherwise → Idle.

**SSE approach (future):**

Connect to `GET http://127.0.0.1:<port>/event` and maintain a local state cache, updated in real time. More efficient for continuous monitoring (e.g., `amux status` in the tmux statusline) but more complex to implement.

## Impact on PRD / Todos

The PRD's status model is mostly achievable with the following caveats:

1. **"Completed" status is not directly available.** OpenCode sessions go from `busy` → `idle` when done, but there's no flag distinguishing "finished task" from "never started." For MVP, **Idle covers both** — we should simplify the status enum to drop Completed, or mark it as a future stretch goal that requires tracking state transitions over time.
2. **"Awaiting Input" requires a second API call** (to `/permission`) beyond the session status endpoint. This is still straightforward but worth noting architecturally — it's not a single status field.
3. **Port discovery is non-trivial.** There's no on-disk state file. We need process-level port detection (`lsof`), which is platform-specific. This adds complexity to the tmux integration layer.
4. **The HTTP API requires a `directory` header** for project-scoped endpoints. We'll need to determine the working directory of the opencode process (from `/proc/<pid>/cwd` on Linux, or `lsof -p <pid> | grep cwd` on macOS).
