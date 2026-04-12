# PRD-007 — `st-send` Session-Level Target Resolution

**Status:** Implemented (v0.10.1)
**Target version:** v0.10.1
**Scope:** `apps/steop/internal/client/resolve.go` — target resolution logic
**Author:** Tasanakorn (design) + Claude Code (PRD authoring)

---

## 1. Goals

1. **Resolve to the actual active session.** `steop send <target>` scans active sessions, finds the best match by `project_dir`, and should use that session's full composite ID (`host:project_dir:UUID`) as the `to` address. Currently it discards the matched session's ID and recomposes with the literal `USER`, which is not a real session address.
2. **Fallback to project-level address.** If no non-USER active session is found for the matched `project_dir`, resolve to the 2-segment `host:project_dir` instead of erroring. This allows messages to be queued for a project that has no active session yet.

## 2. Non-goals

- Changing the server-side `steop_mailbox_list` query to support prefix matching or fan-out delivery.
- Multi-session fan-out (sending to all sessions of a project simultaneously).

## 3. Background & Motivation

PRD-006 introduced `steop send` with short-name resolution via `ResolveTarget()` in `resolve.go`. The algorithm correctly finds active sessions by suffix-matching `project_dir` and picks the most recent by `last_active_at` — but then discards the matched session's composite ID and recomposes with the literal `USER`:

```go
// resolve.go:64
return ComposeSessionID(c.Host(), best.projectDir, "USER"), nil
```

This command is designed to send messages to active sessions, not to `USER`. The resolution should return the session ID it already found. When no real session exists, it should fall back to project-level addressing (`host:project_dir`).

The watcher (`st-watch`) polls using its own session ID, which the PreToolUse hook injects automatically via `--x-session-id`. No changes are needed on the watcher side.

## 4. Design

### 4.1 Resolution logic

The `match` struct currently only tracks `projectDir` and `lastActiveAt`. Add the session's full composite ID and return it directly instead of recomposing with `USER`.

```go
type match struct {
    id           string    // full composite ID from session.list (host:project_dir:UUID)
    projectDir   string
    lastActiveAt time.Time
}
```

During iteration, skip entries that are not real sessions (missing 3rd segment or `USER` segment). Track matched `projectDir` values separately so we can still detect ambiguity and produce a fallback:

```go
parts := strings.SplitN(s.ID, ":", 3)
if len(parts) < 2 {
    continue
}
projectDir := parts[1]
if projectDir != target && !strings.HasSuffix(projectDir, "/"+target) {
    continue
}
seen[projectDir] = true
// Only include real sessions (non-USER) as candidates
if len(parts) < 3 || parts[2] == "USER" {
    continue
}
t, _ := time.Parse(time.RFC3339, s.LastActiveAt)
matches = append(matches, match{id: s.ID, projectDir: projectDir, lastActiveAt: t})
```

### 4.2 Return value

**If real sessions found:** return the best match's composite ID directly:

```go
return best.id, nil
```

**If no real sessions but `project_dir` matched:** fall back to 2-segment project-level address:

```go
// seen has exactly one projectDir (ambiguity already checked above)
for dir := range seen {
    return c.Host() + ":" + dir, nil
}
```

**If nothing matched at all:** error as before.

### 4.3 Full composite ID passthrough (unchanged)

When the target contains `:`, it is returned as-is. No change to this path.

## 5. Changes by Component

| Component                                     | Change                                                                                |
| --------------------------------------------- | ------------------------------------------------------------------------------------- |
| `apps/steop/internal/client/resolve.go`       | Add `id` to `match` struct; filter out `USER`; return `best.id` or 2-segment fallback |
| `docs/prd/prd-007-st-send-session-resolve.md` | This PRD                                                                              |
| `docs/README.md`                              | Add PRD-007 row to the PRD table                                                      |

## 6. Edge Cases

| Scenario                                       | Behavior                                                                               |
| ---------------------------------------------- | -------------------------------------------------------------------------------------- |
| Session with `:USER` segment                   | Skipped as candidate — not a real session                                              |
| Only USER sessions match the project_dir       | Fall back to 2-segment `host:project_dir`                                              |
| Multiple sessions for same project_dir         | Pick most recent by `last_active_at`, use that session's full composite ID             |
| Target session stops between resolve and send  | Message stays NEW until a new session starts; no different from any other timing race   |
| Full composite ID passthrough                  | Unchanged — user can pass a full `host:project:UUID` directly, skipping resolution     |

## 7. Migration

No migration required. Client-side change only. The `steop_mailbox` table, server API, and wire protocol are unchanged.

## 8. Testing

### 8.1 Manual smoke test

```bash
# Start st-watch in one session (e.g. stele-monitor project)
# In another session:
steop send stele-monitor "ping test"

# Verify output shows a UUID in the 3rd segment, not "USER":
#   sent task to tas-mbp4:/Users/.../stele-monitor:<UUID> (message_id: ..., mode: normal)

# Verify st-watch in the target session receives and processes the message.
```

### 8.2 Unit test (if test infra exists)

- Mock `session.list` returning a session with ID `host:project:abc-123-uuid`, verify `ResolveTarget` returns `host:project:abc-123-uuid`.
- Mock `session.list` returning only a `USER` entry for the project, verify `ResolveTarget` returns the 2-segment `host:project`.
- Mock `session.list` returning no matches, verify error.
