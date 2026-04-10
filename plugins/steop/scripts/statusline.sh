#!/usr/bin/env bash
# Claude Code statusline template — shipped by the steop plugin.
#
# Ported from cerbrix's scripts/statusline.sh (same author) with colors
# tuned for dark backgrounds and a steop line-2 block added at the bottom.
# `/steop:statusline-setup` copies this file to ~/.claude/statusline.sh.
#
# Line 1: model | project | git branch | context bar | cost or rate limits
# Line 2: steop phase/step/counters (prints nothing when unavailable)
#
# Configure in ~/.claude/settings.json:
#   "statusLine": {"type": "command", "command": "bash ~/.claude/statusline.sh"}
#
# Requires: jq
# Optional: steop (appends phase/step/counters on line 2 when active)
#
# To disable a segment, comment out its block below. This file is yours —
# edit freely. Re-running /steop:statusline-setup overwrites it (with a
# .bak), so move customisations to a different filename to preserve them.

set -euo pipefail

# --- ANSI colors ---------------------------------------------------------
# Bright set (9x) for dark-background legibility. No dim (2m). No bold —
# bold-bright tends to render as chunky near-white on many terminals and
# loses the subtle hierarchy we want. No bright blue (94m) — it's the one
# 16-color code most commonly rendered hard-to-read on dark themes. Set
# NO_COLOR=1 to disable.
if [ -z "${NO_COLOR:-}" ]; then
  C_RESET=$'\033[0m'
  C_SEP=$'\033[37m'       # muted white separator
  C_MODEL=$'\033[96m'     # bright cyan       — primary identifier
  C_PROJECT=$'\033[97m'   # bright white      — secondary (safe on any dark theme)
  C_BRANCH=$'\033[92m'    # bright green      — conventional git green
  C_BAR_OK=$'\033[92m'    # bright green      (  0-60% )
  C_BAR_WARN=$'\033[93m'  # bright yellow     ( 60-85% )
  C_BAR_HOT=$'\033[91m'   # bright red        ( 85+%   )
  C_PCT=$'\033[97m'       # bright white      — the numeric next to the bar
  C_RATE=$'\033[96m'      # bright cyan       — rate-limit label (cyan, not magenta)
  C_COST=$'\033[93m'      # bright yellow     — money
else
  C_RESET=''; C_SEP=''; C_MODEL=''; C_PROJECT=''; C_BRANCH=''
  C_BAR_OK=''; C_BAR_WARN=''; C_BAR_HOT=''; C_PCT=''
  C_RATE=''; C_COST=''
fi

# --- Read session JSON from stdin ---
SESSION=""
if [ ! -t 0 ]; then
  SESSION=$(cat)
fi

jval() { echo "$SESSION" | jq -r "$1 // empty" 2>/dev/null; }

# elapsed_pct <resets_at_epoch> <window_seconds>
# Echoes the integer percent (0–100) of the window that has elapsed, where
# `resets_at` is the Unix epoch timestamp at which the window next resets.
# Emits nothing when resets_at is missing or non-numeric, so callers can
# test `[ -n "$result" ]`. Clamps to [0, window] to absorb clock skew.
elapsed_pct() {
  local resets_at=$1
  local window=$2
  resets_at="${resets_at%%.*}"
  case "$resets_at" in
    ''|*[!0-9]*) return 0 ;;
  esac
  local now window_start elapsed
  now=$(date +%s)
  window_start=$((resets_at - window))
  elapsed=$((now - window_start))
  [ "$elapsed" -lt 0 ] && elapsed=0
  [ "$elapsed" -gt "$window" ] && elapsed=$window
  awk "BEGIN {printf \"%.0f\", ($elapsed / $window) * 100}"
}

line1=()

# --- Segment: Model ---
model=$(jval '.model.display_name')
[ -z "$model" ] && model=$(jval '.model.id')
[ -n "$model" ] && line1+=("${C_MODEL}${model}${C_RESET}")

# --- Segment: Project directory ---
project=$(jval '.workspace.project_dir')
[ -n "$project" ] && line1+=("${C_PROJECT}$(basename "$project")${C_RESET}")

# --- Segment: Git branch ---
branch=$(git branch --show-current 2>/dev/null || true)
[ -n "$branch" ] && line1+=("${C_BRANCH}${branch}${C_RESET}")

# --- Segment: Context window ---
# Bar color shifts green → yellow → red as the context fills up.
ctx_pct=$(jval '.context_window.used_percentage')
if [ -n "$ctx_pct" ]; then
  width=8
  filled=$(awk "BEGIN {printf \"%d\", ($ctx_pct/100)*$width}")
  [ "$filled" -gt "$width" ] && filled=$width
  empty=$((width - filled))
  bar=""
  i=0; while [ $i -lt "$filled" ]; do bar="${bar}█"; i=$((i+1)); done
  i=0; while [ $i -lt "$empty" ]; do bar="${bar}░"; i=$((i+1)); done
  bar_color="$C_BAR_OK"
  pct_int=$(printf '%.0f' "$ctx_pct")
  if [ "$pct_int" -ge 85 ]; then
    bar_color="$C_BAR_HOT"
  elif [ "$pct_int" -ge 60 ]; then
    bar_color="$C_BAR_WARN"
  fi
  line1+=("${bar_color}${bar}${C_RESET} ${C_PCT}${pct_int}%${C_RESET}")
fi

# --- Segment: Cost / Rate limits (quota used vs time elapsed) ---
# OAuth subscribers (Pro/Max) get rate_limits; API users get cost.
#
# For each rate-limit window (5h, 7d), displays `NNh:used%/elapsed%`, where
# `elapsed%` is how far through the window we currently are (computed from
# `resets_at`). Single comparative rule:
#   used > elapsed → yellow (burning quota faster than the clock)
#   otherwise      → green  (on track)
# When a `resets_at` is missing for a window, that row falls back to
# uncolored `NNh:used%` — we can't run the burn-rate check without a clock.
rate_5h=$(jval '.rate_limits.five_hour.used_percentage')
if [ -n "$rate_5h" ]; then
  rate_5h_int=$(printf '%.0f' "$rate_5h")
  time_5h=$(elapsed_pct "$(jval '.rate_limits.five_hour.resets_at')" 18000)
  if [ -n "$time_5h" ]; then
    if [ "$rate_5h_int" -gt "$time_5h" ]; then color_5h="$C_BAR_WARN"; else color_5h="$C_BAR_OK"; fi
    line1+=("${C_RATE}5h:${C_RESET}${color_5h}${rate_5h_int}%/${time_5h}%${C_RESET}")
  else
    line1+=("${C_RATE}5h:${C_RESET}${C_PCT}${rate_5h_int}%${C_RESET}")
  fi

  rate_7d=$(jval '.rate_limits.seven_day.used_percentage')
  if [ -n "$rate_7d" ]; then
    rate_7d_int=$(printf '%.0f' "$rate_7d")
    time_7d=$(elapsed_pct "$(jval '.rate_limits.seven_day.resets_at')" 604800)
    if [ -n "$time_7d" ]; then
      if [ "$rate_7d_int" -gt "$time_7d" ]; then color_7d="$C_BAR_WARN"; else color_7d="$C_BAR_OK"; fi
      line1+=("${C_RATE}7d:${C_RESET}${color_7d}${rate_7d_int}%/${time_7d}%${C_RESET}")
    else
      line1+=("${C_RATE}7d:${C_RESET}${C_PCT}${rate_7d_int}%${C_RESET}")
    fi
  fi
else
  cost=$(jval '.cost.total_cost_usd')
  if [ -n "$cost" ] && [ "$cost" != "0" ]; then
    line1+=("${C_COST}$(printf '$%.2f' "$cost")${C_RESET}")
  fi
fi

# --- Output line 1 ---
join() {
  local result=""
  local sep="${C_SEP} | ${C_RESET}"
  for p in "$@"; do
    [ -n "$result" ] && result="${result}${sep}"
    result="${result}${p}"
  done
  printf '%s\n' "$result"
}

[ ${#line1[@]} -gt 0 ] && join "${line1[@]}"

# --- Line 2: steop pipeline state ---
# Rendered by the steop companion binary. If steop is not on PATH (or the
# stele-server is unreachable) this block prints nothing and the statusline
# degrades to a single line — nothing breaks.
if command -v steop >/dev/null 2>&1; then
    steop statusline 2>/dev/null || true
fi
