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
Title:             <working title>
Scope:             <affected components — 1 line>
Assumptions:       <0–3 bullets — list explicitly; do NOT investigate to remove>
Goals:             <1–3 bullets>
Non-goals:         <1–3 bullets>
Success criteria:  <1–3 bullets — what would make this PRD "done" after authoring
                    AND after the implementing PRD ships>
Version:           <vX.Y.Z>
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
- **Present alternatives when the request has two or more plausible framings.** If a concrete request admits multiple concrete interpretations (e.g. "apply X to the skills" could mean rewrite one skill, edit several, or add a new enforcement skill), present all plausible framings in the Brief step and ask which. Do NOT pick silently. This is distinct from "vague" — the request is concrete, but multiple concrete interpretations satisfy it.
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
> - **Section template (canonical order — top-level headings unchanged):** Goals, Non-goals, Background & Motivation (with "Current state" subsection), Design, Changes by Component (table), Edge Cases, Migration, Testing. **Omit sections with nothing real to put in them** — if the PRD has no migration, drop the `## 7. Migration` heading rather than writing "None." If the PRD has no non-trivial edge cases, drop `## 6. Edge Cases`. Never write placeholder prose for the sake of structural completeness.
> - **Testing section format:** Each testable outcome written as `step → verify:` — a concrete action on the left, an observable signal on the right. The top-level heading stays `## N. Testing` for style continuity with prior PRDs.
> - **Alternatives considered:** For each major design choice, include one short line — inline in the relevant `### 4.x` subsection — noting a simpler alternative considered and why it was rejected. Format: `**Alternative considered:** <one line>. Rejected: <one line>.` Do NOT add a standalone `## Alternatives` section; a single summary section is too coarse and breaks surgical-change flow.
> - **Style match:** Read the most recent 2–3 PRDs in `docs/prd/` (sorted by filename) before you write. Match their prose voice (declarative, not hortative), heading depth (top-level `##`, subsection `###`, rarely `####`), admonition conventions (blockquote `> **Superseded by ...**` when applicable), and table column styles. The PRD you write should be visually indistinguishable from its neighbors.
> - **Status:** `Proposed` unless the user specified otherwise during Clarify.
> - **README update:** Add a row to the PRD table in `docs/README.md` with the new PRD link, status, and one-line description. Do NOT touch adjacent rows, even to fix known drift. One concern per PRD.
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
