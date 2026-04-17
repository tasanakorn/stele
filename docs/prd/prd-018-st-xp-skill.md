# PRD-018 — `/steop:st-xp` Skill (XP-Style Fast-Feedback Workflow)

**Status:** Implemented (v0.14.0)
**Target version:** v0.14.0
**Scope:** `plugins/steop/` — new skill only (no new agents, no binary/hook changes)
**Author:** Tasanakorn (design) + Claude Code (PRD authoring)

> **Superseded by [PRD-024](prd-024-rename-st-xp-to-st-lite.md) (v0.19.0).** The skill is renamed from `/steop:st-xp` to `/steop:st-lite`; mode tag flips from `--mode xp` to `--mode lite`. Pipeline behavior is unchanged.

---

## 1. Goals

1. **Add a compressed 3-phase pipeline** (`Clarify → Execute → Validate`, tagged `--mode xp`) behind `/steop:st-xp`, optimized for speed and fast feedback over thoroughness. Phase names match `st-flow` for cognitive consistency; the `[xp]` mode tag signals the compressed semantics.
2. **Smallest-slice-first by default.** Prefer assumptions + YAGNI; ship the smallest working slice and let reality correct it rather than pre-designing every edge case.
3. **Eager parallel execution on independent groups** during Execute, capped at **3 concurrent agents** to keep orchestration overhead tractable.
4. **Parallel exploration is opt-in**, not default — fan-out of multiple competing approaches only triggers when Clarify flags approach-ambiguity **or** the user passes an explicit `--explore` hint.
5. **Single (serial) Validate pass** at the end — one reviewer pass for clarity, not exhaustive validation.

## 2. Non-goals

- Replacing `/steop:st-flow`. XP is a peer skill for a different complexity profile, not a successor.
- Creating new agent files. Reuses `steop:consultant`, `steop:executor`, `steop:reviewer` with XP-mode override blockquotes.
- Changing the `steop` binary or hook logic. `state set-phase --mode xp` works today (mode is a free-form string — see `apps/steop/cmd_state.go:105-130`).
- Auto-updating PRD status on completion. XP is exploratory — finish states are intentionally not treated as "implemented."
- Introducing a retry loop. Validate failures fail-fast (see §7).

## 3. Background & Motivation

### 3.1 Current state — `st-flow` and `st-prd` cost profiles

| Skill            | Phases                                              | Agents                | Typical turns | Retry loop | Feedback speed |
| ---------------- | --------------------------------------------------- | --------------------- | ------------- | ---------- | -------------- |
| `/steop:st-flow` | Clarify → [Research] → Plan → Execute → Validate    | 4–5 (opus + sonnet)   | 8–15          | Yes (×3)   | Slow (minutes) |
| `/steop:st-prd`  | Clarify (interactive) → Research → Author           | 2 (researcher + arch) | 5–10          | No         | Medium         |
| *(gap)*          | —                                                   | —                     | —             | —          | Fast (seconds) |

Both existing skills optimize for **correctness** and **completeness**: deep research, formal planning, exhaustive validation. That's the right default for production changes. But a class of requests doesn't need it:

- "Try adding this flag and see if it compiles."
- "Prototype a different storage layout."
- "Quick refactor — rename this and fix the call sites."
- "Experiment: does this API shape feel right?"

For these, the overhead of a full flow (Clarify brief, Research agent, Plan blueprint, Validate retry loop) costs more than the task itself. The user wants **just try it** — land a working slice fast, learn from it, iterate.

### 3.2 Why XP

"Extreme Programming" style: minimize ceremony, maximize feedback. Assume-and-verify instead of research-and-prove. The value proposition is **wall-clock time to first runnable change** — typically <30 seconds of orchestration before the executor is running, instead of minutes.

XP is complementary, not competitive, with `st-flow`:

- Prototype with `st-xp`, harden with `st-flow`.
- Simple refactors with `st-xp`, architectural changes with `st-flow`.
- Exploration with `st-xp`, PRD implementation with `st-flow`.

## 4. Design

### 4.1 Phase-by-phase walkthrough

Phase names mirror `/steop:st-flow` (`Clarify / Execute / Validate`) so users carry one mental model across skills. The `--mode xp` tag, visible in the statusline as `[xp]`, signals the compressed semantics (compressed phases, sonnet-biased, fail-fast, no retry). Clarify still uses **opus** because scope decomposition and complexity assessment need real reasoning — the cost savings in XP come from skipping Research/Plan, not from underpowering Clarify.

#### Phase 1 — Clarify

```bash
steop state set-phase clarify --mode xp
```

Launch the **consultant** agent (**opus** — scope/complexity/group decomposition needs real reasoning even in XP mode). Pass XP-mode override blockquote:

> **XP MODE:** Produce a minimal brief in 1–3 tool calls max. Do NOT ask questions. Emit only: 1-line objective, approach confidence (`high` or `ambiguous`), complexity guess (`trivial`/`moderate`/`complex`). If complexity=`complex`, append a one-line suggestion to consider `/steop:st-flow` instead — but still proceed. Prefer assumptions over investigation.

Output shape (roughly):

```
Objective:   <one line>
Approach:    high | ambiguous
Complexity:  trivial | moderate | complex
Groups:      [G1: files/area, G2: ..., G3: ...]   # only if independent
```

Emit status: `[xp] Clarify: <objective>`

#### Phase 2 — Execute

```bash
steop state set-phase execute --mode xp
```

Launch **executor** agent(s). Model defaults to **sonnet**; downgrade to **haiku** only when Clarify emits `complexity=trivial` (pure renames, single-flag toggles, mechanical edits). Override blockquote:

> **XP MODE:** Implement the smallest working slice. Prefer YAGNI — skip defensive code, edge cases, and polish unless they block the happy path. Leave TODOs where assumptions are load-bearing. Do NOT refactor neighboring code. Return as soon as the main path works.

**Parallel fan-out** (cap 3):

- If Clarify emitted independent `Groups:`, launch one executor per group (up to 3 in parallel).
- If Clarify emitted `approach: ambiguous` **OR** user passed `--explore`, launch up to 3 executors on the **same** group with different approaches — this is "parallel exploration" mode. Caller picks the best result in the Finalize block.
- Otherwise (default): single executor.

Emit status: `[xp] Execute: <N> parallel` (or `[xp] Execute: 1` for single).

#### Phase 3 — Validate

```bash
steop state set-phase validate --mode xp
```

Launch **reviewer** agent (**sonnet** — final correctness signal deserves a capable model; haiku too shallow for catching subtle regressions). Single serial pass — no fan-out. Override blockquote:

> **XP MODE:** Lightweight smoke check only. Does it build? Does the main path run? Are there obvious regressions in touched files? Do NOT audit exhaustively, do NOT run full test suites unless the project has a fast `make check` or equivalent. Report `pass` or `fail` with a one-line reason.

Emit status: `[xp] Validate: <pass|fail>`

No retry loop. On `fail`, halt and report — the user decides whether to iterate manually, rerun XP with adjusted scope, or escalate to `st-flow`.

### 4.2 Agent override table

| Phase    | Agent              | Model (default) | Override trigger                | Tools                  | Parallel?             |
| -------- | ------------------ | --------------- | ------------------------------- | ---------------------- | --------------------- |
| Clarify  | `steop:consultant` | opus            | —                               | Glob, Grep, Read, Bash | No                    |
| Execute  | `steop:executor`   | sonnet          | haiku if `complexity=trivial`   | All tools              | Yes (cap 3)           |
| Validate | `steop:reviewer`   | sonnet          | —                               | Glob, Grep, Read, Bash | No (serial by design) |

Model overrides are applied via the agent invocation `model:` field, matching the pattern already used in `st-flow`.

### 4.3 Parallel execution model

- **Cap:** 3 concurrent agents during Execute. Enforced by the skill's fan-out logic — do not launch a 4th even if Clarify emits more groups (merge the tail into the last group or defer with a TODO).
- **Independent groups (default fan-out):** multiple executors work on **disjoint** file sets. Results merge trivially.
- **Parallel exploration (opt-in):** multiple executors work on the **same** problem with different approaches. Results compete; the Finalize block surfaces all 3 and the user picks one.
- **Validate is always serial.** Parallelizing review adds coordination overhead without meaningful speedup on a single slice, and serial review produces a single clear pass/fail verdict.

### 4.4 Comparison — `st-flow` vs `st-xp`

| Dimension              | `/steop:st-flow`                                 | `/steop:st-xp`                                     |
| ---------------------- | ------------------------------------------------ | -------------------------------------------------- |
| Phases                 | Clarify → [Research] → Plan → Execute → Validate | Clarify → Execute → Validate                       |
| Phase count            | 4–5                                              | 3 (Research + Plan removed/merged)                 |
| Agent count            | 4–5                                              | 3 (all reused)                                     |
| Default model          | opus (clarify/plan) + sonnet (exec/val)          | opus (clarify) + sonnet (exec/val); haiku if trivial |
| Retry loop             | Yes (execute↔validate up to 3×)                  | No — fail-fast, report and halt                    |
| Parallel fan-out       | Optional (researchers, executors)                | Default (cap 3 executors)                          |
| Parallel exploration   | No                                               | Opt-in (`--explore` or `approach: ambiguous`)      |
| Statusline mode        | `[flow]`                                         | `[xp]`                                             |
| PRD status auto-update | Yes (on validation pass)                         | No (XP is exploratory)                             |
| Target use             | Production changes, architectural work           | Prototypes, refactors, spikes, experiments         |
| Typical wall time      | Minutes                                          | Seconds to a minute                                |

## 5. Changes by Component

| Component                                       | Change type | Description                                                                                    |
| ----------------------------------------------- | ----------- | ---------------------------------------------------------------------------------------------- |
| `plugins/steop/skills/st-xp/SKILL.md`           | New         | New skill file — 3 phases, agent override blockquotes, cap-3 parallel rules, Finalize template |
| `plugins/steop/.claude-plugin/plugin.json`      | Modified    | Version bump `0.13.3` → `0.14.0` (handled by `scripts/bump-version.py` lock-step)              |
| `apps/stele/Cargo.toml`                         | Modified    | Workspace version `0.13.3` → `0.14.0` (lock-step)                                              |
| `plugins/stele/.claude-plugin/plugin.json`      | Modified    | Version `0.13.3` → `0.14.0` (lock-step per `docs/versioning.md`)                               |
| `docs/README.md`                                | Modified    | Add PRD-018 row to PRD table                                                                   |
| `plugins/steop/README.md`                       | Modified    | List `/steop:st-xp` under Skills section                                                       |
| `CLAUDE.md`                                     | Modified    | Add `/steop:st-xp` bullet to the Steop Plugin → Skills list                                    |

No changes to `apps/steop/` Go sources. No new agent files under `plugins/steop/agents/`. No hook changes.

## 6. Edge Cases

| Scenario                                                               | Behavior                                                                                                                                      |
| ---------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------- |
| Clarify emits `approach: ambiguous` but user did NOT pass `--explore`  | Default to single best-guess approach. Note the ambiguity in the Finalize "Known-unknowns" block so the user can rerun with `--explore`.       |
| Execute fails mid-run (tool error, build break)                        | Fail-fast. Report partial state — which files were modified by which executor, where it stopped. No auto-rollback. User decides next step.     |
| User invokes `/steop:st-xp` on a genuinely complex task                | Clarify emits `complexity=complex` and appends a "consider `/steop:st-flow` instead" suggestion. XP still proceeds if user doesn't intervene.  |
| Parallel executors touch overlapping files                             | Last-write-wins at the filesystem level. Skill should detect overlap during Clarify (groups must be disjoint) — if detected, collapse to 1 exec. |
| Identity injection in parallel executors                               | Same as `st-flow`: PreToolUse hook injects identity only for Bash-launched `steop` calls. Nested subprocesses must pass `--session-id` / `--project-dir` explicitly. |
| User passes `--explore` on a trivial task                              | Still respected — run up to 3 competing executors. Overkill but not wrong; trust the explicit signal.                                          |
| Validate reports `fail`                                                | Halt pipeline. Emit `[xp] Validate: fail` statusline and Finalize with failure summary. No retry.                                              |
| Stop conditions                                                        | Same as `st-flow`: same error 3× during Execute; user says "stop"/"cancel"/"pause"; Validate reports any issue → halt.                         |

## 7. Migration

- **Purely additive.** New skill, no removals, no semantic changes to existing skills.
- **Existing `st-flow` / `st-prd` users unaffected.** Nothing they rely on changes.
- **Plugin version bump is lock-step** with workspace version per `docs/versioning.md` — `scripts/bump-version.py 0.14.0` handles `apps/stele/Cargo.toml`, both plugin manifests, and any cross-refs in one shot.
- **No database migrations.** No new storage keys. No schema changes.

## 8. Testing

Manual smoke tests (no automated coverage yet — consistent with existing skills):

1. **Trivial refactor** — `/steop:st-xp rename this helper and fix call sites`. Verify Clarify is lightweight (≤3 tool calls), Execute runs a single executor, Validate passes. Expect <30s orchestration overhead.
2. **Prototype/spike request** — `/steop:st-xp prototype a different storage layout for X`. Verify it runs without asking questions and produces a runnable slice.
3. **Complex task (warning path)** — `/steop:st-xp refactor the whole mailbox module to use channels`. Verify Clarify emits `complexity=complex` and the "consider st-flow" suggestion; skill still proceeds.
4. **Parallel exploration** — `/steop:st-xp --explore add retry logic to X`. Verify up to 3 executors launch in parallel, each with a different approach; Finalize surfaces all 3.
5. **Parallel groups (default)** — request that spans 2–3 independent files. Verify cap-3 executor fan-out and disjoint file ownership.
6. **State transitions** — after each phase, run `steop state get` (or observe statusline) and verify mode renders as `[xp]` and phase is `clarify`/`execute`/`validate`.
7. **Fail-fast behavior** — introduce a deliberate build break during Execute. Verify Validate reports `fail` and the pipeline halts without retry.
8. **Stop conditions** — mid-Execute, user types "stop". Verify clean halt.

## 9. Open Questions

1. Should the skill accept a `--cap N` flag to tune the parallel cap below 3 (e.g. `--cap 1` for sequential-only)? Deferred — users can just omit `--explore` and let Clarify emit a single group to get sequential behavior today.
2. Should XP mode automatically archive its workspace diff so the user can easily revert a failed run? Deferred — `git stash` is the lighter-weight answer; only revisit if users ask.
3. Should the statusline surface the parallel executor count dynamically (e.g. `[xp] Execute: 2/3 running`)? Deferred — requires statusline JSON shape extension; current `[xp] Execute: <N> parallel` one-shot is sufficient for v0.14.0.
