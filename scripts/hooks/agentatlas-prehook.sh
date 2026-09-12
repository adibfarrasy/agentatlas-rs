#!/usr/bin/env bash
# scripts/hooks/agentatlas-prehook.sh — Claude Code PreToolUse hook.
#
# Before the agent runs a retrieval tool (Bash/Read/Grep/Glob), inject a one-line nudge to reach
# for `agentatlas` first instead of blind grep + whole-file reads. Non-blocking: exits 0, never
# denies a call — it only adds context to the model's next turn.
#
# Installer places this next to agentatlas-reminder.txt in the agent's hooks dir.

set -eu

self_dir="$(cd "$(dirname "${BASH_SOURCE[0]:-$0}")" && pwd)"
reminder="$self_dir/agentatlas-reminder.txt"
[ -f "$reminder" ] || exit 0

input="$(cat 2>/dev/null || true)"
[ -n "$input" ] || exit 0

tool="$(printf '%s' "$input" | sed -n 's/.*"tool_name"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/p' | head -1)"
case "$tool" in
    Bash|Read|Grep|Glob) ;;
    *) exit 0 ;;
esac

# join reminder to one space-separated line, then JSON-escape quotes and backslashes (no jq/python)
text="$(tr '\n' ' ' < "$reminder")"
escaped="$(printf '%s' "$text" | sed -e 's/\\/\\\\/g' -e 's/"/\\"/g')"

printf '{"hookSpecificOutput":{"hookEventName":"PreToolUse","additionalContext":"%s"}}\n' "$escaped"
exit 0