#!/bin/sh
# amux-status v1.0 — Gemini CLI extension hook
# Writes agent status files for amux. No stdout (Gemini CLI hook protocol).

# No-op outside tmux.
[ -z "$TMUX_PANE" ] && exit 0

# Read hook input from stdin.
input=$(cat)

# Extract hook_event_name.
hook_event_name=$(printf '%s' "$input" | grep -o '"hook_event_name" *: *"[^"]*"' | sed 's/.*: *"//;s/"//')

# Determine status directory.
status_dir="${XDG_STATE_HOME:-$HOME/.local/state}/amux/gemini"
status_file="$status_dir/$TMUX_PANE.json"

# SessionEnd: remove status file and exit.
if [ "$hook_event_name" = "SessionEnd" ]; then
    rm -f "$status_file"
    exit 0
fi

# Map event to status.
status=""
case "$hook_event_name" in
    BeforeAgent)
        status="busy"
        ;;
    AfterAgent)
        status="idle"
        ;;
    Notification)
        notification_type=$(printf '%s' "$input" | grep -o '"notification_type" *: *"[^"]*"' | sed 's/.*: *"//;s/"//')
        if [ "$notification_type" = "ToolPermission" ]; then
            status="awaiting_input"
        fi
        ;;
esac

# Nothing to write if status is empty.
[ -z "$status" ] && exit 0

# Write status file.
mkdir -p "$status_dir"
ts=$(date +%s)
printf '{"status":"%s","pid":%d,"ts":%d}' "$status" "$PPID" "$ts" > "$status_file"

exit 0
