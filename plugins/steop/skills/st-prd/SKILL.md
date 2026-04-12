---
name: st-prd
description: PRD authoring skill. Interactive clarify phase to shape scope, docs-first research, then convention-correct PRD file creation. Use when the user wants to create a new PRD.
---

# PRD Authoring

Create a new PRD through an interactive clarify dialogue, docs-first research, and convention-correct authoring.

```
Clarify (interactive) → Research (autonomous) → Author (autonomous)
       ↑        ↓
       └─ user ─┘
```

## Phase 1: Clarify (In-Conversation)

```bash
steop state set-phase clarify --mode prd
```

This phase runs **in the main conversation** — no subagent. It is a short dialogue to align on scope before any heavy work.

### Step 1 — Gather Context

Do at most 2–3 quick tool calls:

- List `docs/prd/` to find the highest existing PRD number
- Read `apps/stele/Cargo.toml` for the current workspace version
- Optionally list related PRDs if the topic overlaps an existing one

### Step 2 — Propose a PRD Brief

From the user's description (or by asking if none was given), produce a short brief:

```
Title:      <working title>
Scope:      <affected components — 1 line>
Goals:      <1–3 bullets>
Non-goals:  <1–3 bullets>
Version:    <vX.Y.Z>
```

Present the brief and ask: "Anything to change or add before I dig in?"

### Step 3 — Iterate

If the user adjusts scope, update the brief and re-present. Maximum 3 rounds. If still unresolved, suggest the user provide more detail offline.

### Step 4 — Lock

When the user confirms (e.g. "looks good", "go", "y"), freeze the brief and proceed to Research.

**Rules:**

- Never skip Clarify. Even with a detailed description, show the brief and ask.
- Keep it lightweight — no deep file reads, no schema analysis, no multi-agent spawning.
- If the user's description is vague, ask targeted questions (what component? what problem? breaking change?) rather than guessing.
- If a duplicate PRD covers the same area, mention it and ask: supersede, extend, or cancel?

> **Note:** When invoked from st-flow with a PRD reference (e.g. `st-flow implement per docs/prd/prd-NNN-...`), the PRD itself is a pre-clarified artifact. The flow's Clarify phase should read the PRD, extract the brief, and proceed without interactive questions.

## Phase 2: Research (Autonomous, Docs-First)

```bash
steop state set-phase research --mode prd
```

Launch the **researcher** agent (`steop:researcher`, Sonnet, read-only tools) with a **docs-first strategy**:

1. **Docs layer (always):** Read `docs/` — prior PRDs, DESIGN.md, architecture.md, data-model.md, http-api.md — whatever is relevant to the locked brief. Check for contradictions or superseded designs.
2. **CLAUDE.md (always):** Verify conventions, scope hierarchy, and component boundaries.
3. **Source code (only if needed):** Read source files only when docs don't cover the affected area — e.g. undocumented behavior, missing schema details, or implementation gaps not yet captured in docs.

Pass the locked PRD Brief as context to the researcher. The user does not interact during this phase.

Output: a Research Summary passed to the Author phase.

## Phase 3: Author (Autonomous)

```bash
steop state set-phase author --mode prd
```

Launch the **architect** agent (`steop:architect`, Opus, all tools) with the following override instruction:

> **PRD MODE:** You are writing a PRD document, not an implementation blueprint. Use the PRD Brief and Research Summary to produce a complete PRD file. Follow these conventions exactly:
>
> - **Number allocation:** Scan `docs/prd/` for the highest `NNN` in filenames matching `prd-NNN-*.md`, increment by 1, zero-pad to 3 digits.
> - **Filename:** `prd-NNN-<slug>.md` where slug is derived from the title in kebab-case, max 5 words.
> - **Author field:** Run `git config user.name` to get the name, then format as `<name> (design) + Claude Code (PRD authoring)`.
> - **Section template:** Use this order: Goals, Non-goals, Background & Motivation (with "Current state" subsection), Design, Changes by Component (table), Edge Cases, Migration, Testing.
> - **Status:** `Proposed` unless the user specified otherwise during Clarify.
> - **README update:** Add a row to the PRD table in `docs/README.md` with the new PRD link, status, and one-line description.
>
> After writing, display the file path and a one-line summary. Do NOT ask for approval — the user will review and edit manually.

## Statusline Summary

| Phase    | Status message                             |
| -------- | ------------------------------------------ |
| Clarify  | `[PRD] Clarifying: <working title>`        |
| Research | `[PRD] Researching <N> areas`              |
| Author   | `[PRD] Created docs/prd/prd-NNN-<slug>.md` |

After the Author phase completes:

```bash
steop state clear-phase
```
