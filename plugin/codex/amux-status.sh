#!/bin/sh
# amux-status v1.3 — Codex CLI hook
# Writes agent status files for amux. No stdout (Codex hook protocol).

# No-op outside tmux.
[ -z "$TMUX_PANE" ] && exit 0

# Codex may pass hook input either as argv or stdin depending on version.
if [ -n "$1" ]; then
    input="$1"
else
    input=$(cat)
fi

# Extract the event name. Codex native hooks use hook_event_name; some event
# payloads use type.
hook_event_name=$(printf '%s' "$input" | grep -oE '"hook_event_name"[[:space:]]*:[[:space:]]*"[^"]*"' | sed 's/.*: *"//;s/"//')
if [ -z "$hook_event_name" ]; then
    hook_event_name=$(printf '%s' "$input" | grep -oE '"type"[[:space:]]*:[[:space:]]*"[^"]*"' | sed 's/.*: *"//;s/"//')
fi

status_dir="${XDG_STATE_HOME:-$HOME/.local/state}/amux/codex"
status_file="$status_dir/$TMUX_PANE.json"

status=""
case "$hook_event_name" in
    SessionStart)
        status="idle"
        ;;
    UserPromptSubmit|PreToolUse|PostToolUse|SubagentStart|SubagentStop|Start|task_started)
        status="busy"
        ;;
    Stop|agent-turn-complete|task_complete)
        status="idle"
        ;;
    PermissionRequest|exec_approval_request|apply_patch_approval_request|request_user_input)
        status="awaiting_input"
        ;;
    Error|error|errored)
        status="errored"
        ;;
esac

[ -z "$status" ] && exit 0

find_codex_pid() {
    pid="$PPID"
    while [ -n "$pid" ] && [ "$pid" -gt 1 ] 2>/dev/null; do
        comm=$(ps -o comm= -p "$pid" 2>/dev/null | awk '{print $1}')
        name=${comm##*/}
        case "$name" in
            codex|codex-*)
                printf '%s' "$pid"
                return 0
                ;;
        esac
        pid=$(ps -o ppid= -p "$pid" 2>/dev/null | tr -d ' ')
    done

    # Fallback to the hook parent. This may be short-lived, but is better than
    # omitting the field if Codex changes how it launches hooks.
    printf '%s' "$PPID"
}

mkdir -p "$status_dir"
ts=$(date +%s)
agent_pid=$(find_codex_pid)
# Include the raw event for debugging; amux ignores unknown JSON fields.
printf '{"status":"%s","pid":%d,"ts":%d,"event":"%s"}' "$status" "$agent_pid" "$ts" "$hook_event_name" > "$status_file"

exit 0
