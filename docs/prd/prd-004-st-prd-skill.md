# PRD — `/steop:st-prd` Skill (PRD Authoring)

**Status:** Implemented (v0.9.1)
**Target version:** v0.9.1
**Scope:** steop plugin — new `st-prd` skill, agent, and CLAUDE.md integration
**Author:** Tasanakorn (design) + Claude Code (PRD authoring)

---

## 1. Goals

1. **Fast feedback loop.** The user shapes the PRD interactively through a Clarify conversation before any heavy research or writing happens. Get alignment on scope, goals, and non-goals in under a minute, not after a 10-minute autonomous pipeline.
2. **PRD as pre-clarified input.** A finished PRD contains everything `st-clarify` would produce — scope, goals, non-goals, components, version, design context. When passed to `st-flow`, it reduces the Clarify phase to a quick extraction instead of an interactive loop. The PRD is the bridge between "what do we want?" and "go build it."
3. **Convention-correct output.** Once scope is agreed, the skill handles all mechanical PRD conventions from CLAUDE.md — file naming (`prd-NNN-<slug>.md`), section structure, author field, README update — so the user focuses on design decisions, not formatting.
4. **Docs-first research.** Research prioritizes existing documentation (`docs/`, prior PRDs, CLAUDE.md) over source code. Only read source files when docs lack sufficient context for the topic.

## 2. Non-goals

- Implementing the PRD after writing it. `st-prd` outputs a document; execution is a separate `/steop:st-flow` invocation (or manual work).
- Replacing human design judgment. The skill drafts the PRD; the user reviews, edits, and approves before it's considered final.
- Modifying existing PRDs. This skill creates new PRDs only. Updating a PRD is a manual edit or a separate future skill.
- Auto-committing or auto-merging the PRD file.

## 3. Background & Motivation

### 3.1 Current state

PRD authoring is a manual, multi-step process:

1. Check `docs/prd/` for the highest existing PRD number.
2. Allocate the next number and construct the filename.
3. Copy the structure from a prior PRD (frontmatter, numbered sections, tables).
4. Research the codebase to understand current state and pain points.
5. Write the actual design content.
6. Cross-reference related docs and update `docs/README.md`.

Steps 1–4 are mechanical and error-prone. The conventions are documented in CLAUDE.md but require careful reading. A skill can automate all of these while letting the user focus on the design decisions.

### 3.2 Why a dedicated skill

| Approach                       | Drawback                                                                                   |
| ------------------------------ | ------------------------------------------------------------------------------------------ |
| User writes PRD manually       | Must remember naming, numbering, structure, author format — easy to get wrong              |
| User asks Claude ad-hoc        | No guarantee Claude follows all CLAUDE.md conventions; no codebase research phase          |
| `st-flow` with a PRD prompt    | `st-flow` is optimized for code changes, not document authoring; execute phase writes code  |
| **`st-prd` (this proposal)**   | Purpose-built for PRD authoring; enforces conventions; includes codebase research           |

## 4. Design

### 4.1 Invocation

```
/steop:st-prd <description>
```

Examples:

- `/steop:st-prd add WebSocket push delivery to mailbox`
- `/steop:st-prd` (no description — starts fully interactive)

### 4.2 Core Principle: PRD as Pre-Clarified Artifact

The skill is **interactive by default** — the Clarify phase is a short dialogue to agree on scope before any heavy work.

```
Clarify (interactive) → Research (autonomous) → Author (autonomous)
       ↑        ↓
       └─ user ─┘
```

But the key insight is that a finished PRD **is itself a completed Clarify output**. It contains title, scope, goals, non-goals, affected components, target version, and design rationale — everything `st-clarify` would produce and more. When a PRD is passed to `st-flow`, the flow's Clarify phase should recognize this and reduce itself to a brief extraction rather than an interactive loop.

**Two audiences, one document:**

| Invocation                                          | Clarify behavior                                                |
| --------------------------------------------------- | --------------------------------------------------------------- |
| `/steop:st-prd <topic>`                             | Full interactive loop — propose brief, ask, iterate, lock       |
| `/steop:st-flow implement per docs/prd/prd-NNN-...` | Reduced — read PRD, extract scope as task brief, skip questions  |

A well-structured PRD maps directly to a task brief:

| PRD section       | Task brief field       |
| ----------------- | ---------------------- |
| Title             | Objective              |
| Scope             | Affected components    |
| Goals             | Success criteria       |
| Non-goals         | Out-of-scope guard     |
| Target version    | Version constraint     |
| Changes by Component | File-level plan hint |
| Design            | Implementation context |

This means `st-flow`'s consultant can read the PRD in 1–2 tool calls, emit the task brief, and proceed directly to Research or Plan — no user questions needed. The PRD has already been clarified.

### 4.3 Clarify Phase (Interactive)

The Clarify phase runs **in the main conversation** (no subagent). It is a dialogue, not a one-shot scan.

**Step 1 — Propose a brief.** From the user's description (or by asking if none given), produce a short **PRD Brief**:

```
Title:      <working title>
Scope:      <affected components — 1 line>
Goals:      <1–3 bullets>
Non-goals:  <1–3 bullets>
Version:    <vX.Y.Z>
```

To fill this, do at most 2–3 quick tool calls: check existing PRD numbers in `docs/prd/`, glance at the workspace version in `Cargo.toml`, and optionally list related PRDs. No deep code reading.

**Step 2 — Ask.** Present the brief and ask the user to confirm, adjust, or add context:

> "Here's what I'd scope. Anything to change or add before I dig in?"

**Step 3 — Iterate.** If the user adjusts scope, update the brief and re-present. This loop should be fast — seconds per round, not minutes.

**Step 4 — Lock.** When the user confirms (e.g. "looks good", "go", "y"), freeze the brief and proceed to Research.

**Rules:**
- Never skip Clarify. Even with a detailed description, show the brief and ask.
- Keep it lightweight — no deep file reads, no schema analysis, no multi-agent spawning.
- If the user's description is vague, ask targeted questions (what component? what problem? breaking change?) rather than guessing.
- Maximum 3 clarify rounds before suggesting the user provide more detail offline.

### 4.4 Research Phase (Autonomous, Docs-First)

Runs only after the user locks the brief. A `steop:researcher` agent (Sonnet) gathers context with a docs-first strategy:

1. **Docs layer (always):** Read `docs/` — prior PRDs, DESIGN.md, architecture.md, data-model.md, http-api.md — whatever is relevant to the topic. Check for contradictions or superseded designs.
2. **CLAUDE.md (always):** Verify conventions, scope hierarchy, and component boundaries.
3. **Source code (only if needed):** Read source files only when the docs don't cover the affected area — e.g. undocumented behavior, missing schema details, or implementation gaps not yet captured in docs.

Output: a Research Summary passed to the Author phase. The user does not interact during this phase.

### 4.5 Author Phase (Autonomous)

Writes the PRD file and updates `docs/README.md`. Handles all mechanical conventions:

- **Number allocation:** Scan `docs/prd/` for highest `NNN`, increment, zero-pad.
- **Filename:** `prd-NNN-<slug>.md` (slug from title, kebab-case, max 5 words).
- **Author field:** `<git user.name> (design) + Claude Code (PRD authoring)`.
- **Section template:** Goals, Non-goals, Background, Design, Changes by Component, Edge Cases, Migration, Testing.
- **README update:** Add row to `docs/README.md` PRD table.

After writing, display the file path and a one-line summary. The user reviews and edits manually.

### 4.6 Statusline Integration

| Phase    | Status message                                    |
| -------- | ------------------------------------------------- |
| Clarify  | `[PRD] Clarifying: <working title>`               |
| Research | `[PRD] Researching <N> areas`                     |
| Author   | `[PRD] Created docs/prd/prd-NNN-<slug>.md`        |

## 5. Changes by Component

| Component                                  | Change                                               |
| ------------------------------------------ | ---------------------------------------------------- |
| `plugins/steop/skills/st-prd/SKILL.md`     | New skill — Clarify loop + Research/Author pipeline   |
| `plugins/steop/.claude-plugin/plugin.json`  | Register `st-prd` skill                              |
| `CLAUDE.md`                                | Add `st-prd` to steop skills list                     |
| `docs/README.md`                           | Updated at runtime by the skill (adds PRD row)        |

No new agent file needed — Clarify runs in-conversation, Research reuses `steop:researcher`, Author reuses `steop:architect` with override instructions.

## 6. Interaction Examples

### 6.1 With description

```
User: /steop:st-prd add WebSocket push delivery to mailbox

Claude:
  Title:      WebSocket Push Delivery
  Scope:      stele-server (mailbox module), steop CLI
  Goals:      - Real-time message delivery via WebSocket
              - Replace polling for active sessions
  Non-goals:  - Cross-host transport
              - Auth changes
  Version:    v0.11.0

  Anything to change or add before I dig in?

User: also include fan-out to multiple subscribers

Claude:
  Updated goals:
  Goals:      - Real-time message delivery via WebSocket
              - Replace polling for active sessions
              - Fan-out: one message delivered to multiple subscribers

  Good to go?

User: y

[PRD] Researching 4 areas...
[PRD] Created docs/prd/prd-005-websocket-push.md
  → Review and edit as needed.
```

### 6.2 No description (fully interactive)

```
User: /steop:st-prd

Claude: What feature or change should this PRD cover?

User: some kind of retention policy for old mailbox messages

Claude:
  Title:      Mailbox Message Retention
  Scope:      stele-server (steop_mailbox table, scheduler)
  Goals:      - Auto-archive or delete messages older than a threshold
              - Configurable per-project retention window
  Non-goals:  - Cross-host sync
              - Backup/restore
  Version:    v0.11.0

  Does this match what you have in mind?

User: yeah but make it patch-level, no schema change needed

Claude:
  Updated — Version: v0.10.1 (patch).
  Proceed?

User: go

[PRD] Researching 3 areas...
[PRD] Created docs/prd/prd-005-mailbox-retention.md
```

### 6.3 PRD → st-flow (reduced Clarify)

When a PRD reference is passed to `st-flow`, the Clarify phase extracts scope from the document instead of asking the user:

```
User: /steop:st-prd add retention policies to mailbox
  → Interactive clarify, user shapes scope
  → Creates prd-005-mailbox-retention.md
  → User reviews and approves the PRD

User: /steop:st-flow implement per docs/prd/prd-005-mailbox-retention.md

[Clarify] Read PRD-005: Mailbox Message Retention | Complexity: standard
  → Extracts goals, scope, components, version from PRD
  → Emits task brief (no questions asked)
  → Proceeds immediately to Research

[Research] Investigating 3 areas...
[Plan] 4 steps across 3 files
[Execute] Modified 3 files
[Validate] Pass
```

Compare with a non-PRD invocation where Clarify must ask:

```
User: /steop:st-flow add some retention thing to mailbox
  → Clarify asks: "What retention policy? TTL? Archive? Which messages?"
  → Interactive back-and-forth before proceeding
```

The PRD eliminates this ambiguity upfront — that's its purpose.

## 7. Edge Cases

1. **No description.** Ask what the PRD should cover (fully interactive Clarify).
2. **Duplicate topic.** If an existing PRD covers the same area, mention it during Clarify and ask: supersede, extend, or cancel?
3. **Version ambiguity.** Ask during Clarify — "breaking change? minor or patch?"
4. **User abandons Clarify.** No files are written. No cleanup needed.
5. **Concurrent PRD creation.** Author phase re-scans `docs/prd/` immediately before writing (last-writer-wins, acceptable for local-only tool).

## 8. Migration

No database or server changes. Plugin-only addition — one new skill directory.

## 9. Testing

- **Interactive Clarify:** Invoke with and without description, verify brief is presented and user can adjust.
- **Numbering:** With PRDs 001–004 existing, verify next PRD is 005.
- **README update:** Verify `docs/README.md` gains a new row.
- **Convention compliance:** Author field, slug format, section structure all match CLAUDE.md rules.
- **Abandon test:** Start Clarify, cancel — verify no files are created.
