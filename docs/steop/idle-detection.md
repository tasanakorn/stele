# Detecting Claude Code Idle State

How an external harness — Steop in particular — can decide that a Claude Code session is **truly idle** and not merely between turns. "Truly idle" means: the model is not generating, no subagent is mid-execution, no `run_in_background` Bash spawned earlier in the session is still running, and no foreground monitor / `tail -f` is currently blocking a Bash tool call.

This matters for autonomous workflows (`/steop:st-watch`), scheduled triggers, mailbox routing, and any "wake the user" / "claim the next task" decision. Acting on a false-idle wastes a turn or, worse, races against an in-flight subagent.

> Sources for hook events and statusline payload: official Claude Code docs at <https://code.claude.com/docs/en/hooks> and <https://code.claude.com/docs/en/statusline> (fetched 2026-04-13). Steop's hook wiring lives in `plugins/steop/hooks/hooks.json` and the corresponding handlers in `apps/steop/internal/hooks/`.

---

## TL;DR

There is **no single "idle" hook** that means "Claude is fully done and nothing it spawned is still running." The closest single signal is `Stop` (turn ended) plus the documented `Notification` event with `notification_type == "idle_prompt"` (Claude has been idle for a while waiting on user input). Both fire optimistically — neither knows about background Bash, foreground monitors, or external subprocess fan-out.

The user's suspected `TeammateIdle` event **does exist** but means something narrower: an agent-team teammate is *about to* go idle (multi-agent harness coordination), not "the whole session is quiet." Treat it as a hint, not a verdict.

**Recommended composite detector** (details in [Recommended composite detector](#recommended-composite-detector)):

1. Latch "model done" on `Stop` (and clear it on `UserPromptSubmit` / `PreToolUse`).
2. Maintain a counter of live subagents from `SubagentStart` − `SubagentStop`.
3. Maintain a registry of background-bash PIDs from `PreToolUse(Bash, run_in_background=true)`; reap with `kill -0` or `/proc` polling — Claude Code does not emit a "background process exited" event.
4. Treat the statusline `refreshInterval` tick as an external heartbeat to recompute idle while the model is silent.
5. Declare idle only when **all** of: latch=done, subagent count=0, no live background PIDs, and N seconds have passed since the last `PostToolUse` / `Stop` — pick N from the false-idle tolerance you can stomach (Steop's mailbox watcher uses ~3 s).

---

## Available Signals

Each subsection covers: when it fires, payload shape (only the fields relevant to idle detection), what it tells you, what it does NOT tell you.

### `Stop`

- **When it fires:** Claude finishes generating an assistant turn (the model stops emitting tokens and there is no auto-continuation queued).
- **Payload (relevant fields):**
  ```jsonc
  {
    "session_id":           "<uuid>",
    "transcript_path":      "<path>",
    "cwd":                  "<dir>",
    "hook_event_name":      "Stop",
    "stop_hook_active":     false,                  // true if re-entered from a previous Stop hook that returned a continuation
    "last_assistant_message": "<truncated text>"
  }
  ```
  Steop's `HookInput` carries the same fields (`apps/steop/internal/hooks/input.go`).
- **What it tells you:** the model has yielded the floor. The next event will be either `UserPromptSubmit` (human typed) or `Stop` again after some auto-continuation.
- **What it does NOT tell you:**
  - Whether a `run_in_background: true` Bash spawned earlier is still alive.
  - Whether a foreground Bash currently *running* (e.g. `tail -f`, a long build, a `Monitor` poll) has come back yet — `Stop` does not fire while a tool call is in flight, but "between turns" is not the same as "all side-effects settled."
  - Whether a subagent that returned its result has cleaned up its own background processes.

### `SubagentStop` and `SubagentStart`

- **When `SubagentStart` fires:** a subagent is spawned (e.g. via the Task / Agent tool, or auto-named subagents like `Bash` / `Explore` / `Plan`).
- **When `SubagentStop` fires:** that subagent finishes and returns control to its caller.
- **Payload (`SubagentStop`, relevant fields):**
  ```jsonc
  {
    "session_id":            "<uuid>",
    "hook_event_name":       "SubagentStop",
    "stop_hook_active":      false,
    "agent_id":              "<id>",
    "agent_type":            "<type>",
    "agent_transcript_path": "<path>",
    "last_assistant_message": "<truncated>"
  }
  ```
- **What it tells you:** that *one* subagent thread is done. Pair with `SubagentStart` to maintain a live-subagent counter.
- **What it does NOT tell you:** whether the parent agent is also done — the parent can still be mid-turn waiting on the subagent's result. `Stop` is the parent's signal.

### `Notification`

- **When it fires:** Claude Code emits a UI notification. Critically, it covers idle prompts.
- **Payload:**
  ```jsonc
  {
    "session_id":        "<uuid>",
    "hook_event_name":   "Notification",
    "title":             "<optional>",
    "message":           "<text shown to user>",
    "notification_type": "permission_prompt" | "idle_prompt" | "auth_success" | "elicitation_dialog"
  }
  ```
- **What it tells you (when `notification_type == "idle_prompt"`):** Claude Code's own UI thinks the session has been waiting on user input long enough to nudge the user. This is the strongest "the model thinks it's done waiting" signal the official API gives.
- **What it does NOT tell you:** the threshold and exact trigger conditions for `idle_prompt` are not documented. The other `notification_type` values are not idle signals — `permission_prompt` and `elicitation_dialog` actually mean the model is *blocked on input*, which is a different state from idle (call it "expectant").

### `TeammateIdle`

- **When it fires:** an agent-team teammate is about to go idle. This is part of the multi-agent / Teams harness (the same surface as `TaskCreated`, `TaskCompleted`, `TeamCreate`, `TaskGet`, etc.).
- **Payload (documented):**
  ```jsonc
  {
    "session_id":      "<uuid>",
    "hook_event_name": "TeammateIdle",
    "teammate_name":   "<name>"
  }
  ```
- **What it tells you:** within an agent team, one teammate is winding down and is available to be re-tasked.
- **What it does NOT tell you:** anything about the Steop session itself. Steop is not currently running inside a Teams harness, so this event will not fire in the normal `claude` CLI flow. Document it for completeness but **do not** rely on it for single-session idle detection.

### `UserPromptSubmit`

- **When it fires:** user submits a prompt, before Claude processes it.
- **Payload:** common fields plus `prompt: string`.
- **What it tells you:** an explicit edge — "definitely no longer idle." Use it to clear any latched idle state.

### `PreToolUse` and `PostToolUse`

- **When they fire:** before / after each tool call.
- **Relevant payload (`PreToolUse` for Bash):**
  ```jsonc
  {
    "hook_event_name": "PreToolUse",
    "tool_name":       "Bash",
    "tool_input": {
      "command":            "<shell>",
      "run_in_background":  true,
      "timeout":            <ms>
    }
  }
  ```
  `PostToolUse` includes `tool_response` — for a backgrounded Bash this is the spawn ack, **not** the eventual exit.
- **What they tell you:**
  - `PostToolUse` is the heartbeat for "the model is currently working" — it should reset any idle countdown.
  - Seeing `run_in_background: true` in `PreToolUse(Bash)` is the **only** chance to capture that a long-lived process was just spawned. There is no corresponding "background bash exited" event from Claude Code, so the harness must tee the PID and reap it externally.
  - For foreground Bash (`run_in_background` absent or false), the next `PostToolUse` is the natural completion edge — no extra tracking needed.
- **What they do NOT tell you:** the spawned PID. Steop currently parses this from the command string in `PreToolUse(Bash)` (`apps/steop/internal/hooks/pre_tool_use.go`); a robust implementation has to wrap the user command (e.g. echo `$!` to a sidecar file) because `tool_response` does not carry the PID.

### `SessionStart` and `SessionEnd`

- **`SessionStart`:** session begins or resumes (`source ∈ {startup, resume, clear, compact}`). Use to reset all idle state.
- **`SessionEnd`:** session terminates (`reason ∈ {clear, resume, logout, prompt_input_exit, bypass_permissions_disabled, other}`). Treat as terminal idle — no further activity from this session.
- **What they do NOT tell you:** anything about background processes that outlive the session. A `nohup`-ed bash from earlier in the session can still be running after `SessionEnd`; the harness may want to GC by PID list.

### Statusline command stdin (`statusLine`)

Not a hook, but the most useful **periodic** signal Claude Code exposes.

- **When it fires:** "after each new assistant message, when the permission mode changes, or when vim mode toggles. Updates are debounced at 300ms" — quoted from <https://code.claude.com/docs/en/statusline>. Critically, the same page warns: *"These triggers can go quiet when the main session is idle, for example while a coordinator waits on background subagents."* That is the canonical false-idle scenario.
- **`refreshInterval`:** opt-in field in `~/.claude/settings.json` under `statusLine`. Re-runs the command every N seconds in addition to the event-driven updates. Minimum 1 s. **This is the documented escape hatch** for keeping idle calculations live during quiet periods.
- **Payload (relevant fields):**
  ```jsonc
  {
    "session_id":           "<uuid>",
    "model":                { "id": "<id>", "display_name": "<name>" },
    "cwd":                  "<dir>",
    "workspace": {
      "current_dir":        "<dir>",
      "project_dir":        "<launch dir>",
      "added_dirs":         [],
      "git_worktree":       "<name?>"
    },
    "context_window": {
      "used_percentage":    <number>
    },
    "cost": {
      "total_cost_usd":     <number>
    }
    // additional fields documented at the URL above
  }
  ```
- **What it tells you (when combined with `refreshInterval`):** a clock. Each tick is a chance to recompute "are we still idle?" without depending on Claude to fire a hook. Steop's `cmd_statusline.go` already parses `session_id` and `workspace.project_dir` and resolves them to a Steop session — the same code path can compute and cache an idle verdict.
- **What it does NOT tell you:** anything about subagents, background PIDs, or whether the model is currently generating. The payload is a snapshot of session metadata, not an activity log.

---

## Ambiguity Matrix

For each signal, what it reports under the four scenarios that make "idle" ambiguous. Cells: `fires` = signal arrives, `silent` = no signal, `depends` = behaviour varies (notes inline).

| Signal                                  | True idle (model done, nothing running) | `run_in_background: true` Bash still alive | Foreground monitor command (e.g. `tail -f`) blocking the turn | Subagent currently mid-execution                 |
| --------------------------------------- | --------------------------------------- | ------------------------------------------ | ------------------------------------------------------------- | ------------------------------------------------ |
| `Stop`                                  | fires                                   | fires (false-idle)                         | silent (turn is still in flight)                              | silent (parent waits on subagent result)         |
| `SubagentStop`                          | silent                                  | silent                                     | silent                                                        | fires when *that* subagent ends (others may run) |
| `SubagentStart`                         | silent                                  | silent                                     | silent                                                        | fires on each new subagent                       |
| `Notification` (`idle_prompt`)          | fires after Claude's internal threshold | fires (false-idle, same logic as `Stop`)   | silent                                                        | silent                                           |
| `Notification` (`permission_prompt`)    | silent                                  | silent                                     | depends (only if the blocking command itself prompts)         | depends (subagent may prompt)                    |
| `UserPromptSubmit`                      | silent                                  | silent                                     | silent                                                        | silent                                           |
| `PreToolUse(Bash)`                      | silent                                  | silent                                     | silent (already past the pre-edge)                            | depends (subagent's tool calls may surface)      |
| `PostToolUse`                           | silent                                  | silent (background spawn-ack came earlier) | fires only when the foreground command finally returns        | fires per subagent tool call                     |
| `TeammateIdle`                          | silent (single-session)                 | silent                                     | silent                                                        | depends (only inside a Teams harness)            |
| `SessionEnd`                            | silent (until session actually ends)    | silent                                     | silent                                                        | silent                                           |
| Statusline event-driven (no `refreshInterval`) | silent (per the docs: "triggers can go quiet ... while a coordinator waits on background subagents") | silent                                     | silent                                                        | silent                                           |
| Statusline `refreshInterval` tick       | fires every N s                         | fires every N s (carries no PID info)      | fires every N s                                               | fires every N s                                  |

Read the matrix as: any single row that is `fires` for "True idle" but also `fires` (or `silent`) ambiguously elsewhere is **not** a sufficient idle detector on its own. The only row that disambiguates is the composite — which is why the next section exists.

---

## Recommended composite detector

Pseudocode for the harness. Reset on every `UserPromptSubmit` and `SessionStart`; consult on every `Stop`, every `SubagentStop`, and every statusline `refreshInterval` tick.

```text
state := {
  model_done:        false,        // latched by Stop, cleared by UserPromptSubmit/PreToolUse
  subagent_count:    0,            // SubagentStart++ / SubagentStop--
  bg_pids:           {},           // captured at PreToolUse(Bash, run_in_background=true)
  last_activity_ts:  now(),        // updated on every Pre/PostToolUse, SubagentStart/Stop, UserPromptSubmit
}

on SessionStart:        reset all
on UserPromptSubmit:    state.model_done = false; state.last_activity_ts = now()
on PreToolUse:          state.model_done = false; state.last_activity_ts = now()
                        if tool == Bash and tool_input.run_in_background:
                            pid := capture_via_wrapper(tool_input.command)   // see note below
                            state.bg_pids.add(pid)
on PostToolUse:         state.last_activity_ts = now()
on SubagentStart:       state.subagent_count += 1; state.last_activity_ts = now()
on SubagentStop:        state.subagent_count -= 1; state.last_activity_ts = now()
on Stop:                state.model_done = true
on SessionEnd:          declare TERMINAL_IDLE

# Called from Stop hook, SubagentStop hook, and statusline refresh tick.
def is_truly_idle(state, settle_seconds=3):
    if not state.model_done:                       return false
    if state.subagent_count > 0:                   return false
    state.bg_pids = {p for p in state.bg_pids if pid_alive(p)}   # reap dead PIDs
    if state.bg_pids:                              return false
    if (now() - state.last_activity_ts) < settle_seconds: return false
    return true
```

Notes on the load-bearing assumptions:

- **PID capture for `run_in_background: true` Bash.** Claude Code's `PostToolUse` does not return the spawned PID. Two practical options: (a) wrap the user's command via a Steop-injected prefix that writes `$!` to a known sidecar file under `$CLAUDE_PROJECT_DIR/.steop/bg-pids/<tool_use_id>` (Steop already injects flags in `PreToolUse(Bash)` per PRD-015 / PRD-016, so the inject point exists); (b) externally `pgrep -P <claude_pid>` and diff before/after — racier, but no command rewriting. Path (a) is preferred.
- **`pid_alive`.** `kill -0 <pid>` on POSIX. Treat ESRCH and EPERM-after-recycle as dead.
- **`settle_seconds`.** Empirical. Steop's mailbox watcher polls every ~3 s; matching that avoids "idle, oh wait, not idle" flapping when a `Stop` is followed immediately by an auto-continuation. Bump to 10 s for "wake the user" decisions.
- **Statusline tick is the heartbeat.** Without `refreshInterval`, the harness will not be invoked while the session is quiet — exactly the case where it most needs to recompute. Recommend `refreshInterval: 5` for any settings.json that opts into Steop's autonomous flows. The statusline command itself can be a no-op renderer that only updates the in-memory idle flag.
- **`Notification` with `notification_type == "idle_prompt"`** can be used as a *fast-path* idle confirmation — but still gate on `bg_pids == {} and subagent_count == 0`, because Claude's own idle prompt does not know about background work.

---

## Known gaps / unanswered questions

What the official docs do not tell us, in priority order:

1. **No "background bash exited" event.** `PostToolUse` for a `run_in_background: true` Bash returns at spawn, not at exit. There is no documented hook for the eventual exit. The harness has to track PIDs out-of-band.
2. **`Notification.notification_type == "idle_prompt"` threshold is undocumented.** We do not know how long Claude waits before emitting it, whether it re-fires, or whether it is suppressed inside a tool call. Treat as best-effort.
3. **No payload field exposes whether a tool call is currently in flight.** The model's own `Stop` is the closest proxy, and it cannot fire mid-tool. So "Stop did not arrive" is the only way to know "a tool call is still running" — a silence-as-signal pattern that is fragile across crashes.
4. **`TeammateIdle` semantics outside a Teams harness.** Documented for agent teams, not specified for the standard `claude` CLI. We assume it never fires there but have not verified empirically.
5. **Statusline event-driven cadence during quiet periods.** The docs explicitly warn triggers "can go quiet ... while a coordinator waits on background subagents," but do not enumerate every quiet condition. `refreshInterval` is the only documented mitigation.
6. **No PID, no exit code, no working directory in `Stop` or `SubagentStop` payloads.** Cross-correlating "this Stop ended *that* subagent's work" requires the harness to thread `agent_id` from `SubagentStart`.
7. **`stop_hook_active`'s exact semantics.** Documented as "true if re-entered from a previous Stop hook that returned a continuation" — used to prevent infinite loops when a Stop hook injects new instructions. Not relevant to plain idle detection but worth knowing before any composite detector returns a non-Allow response from `Stop`.
