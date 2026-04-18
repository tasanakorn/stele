# PRD-025 — Karpathy-aligned Brief fields for `/steop:st-prd` and `/steop:st-lite`

- **Status:** Implemented (v0.19.1)
- **Target version:** v0.19.1
- **Scope:** `plugins/steop/skills/st-prd/SKILL.md` — expand Clarify Brief template, rewrite Author override (Karpathy P1/P2/P3/P4 alignment). `plugins/steop/skills/st-lite/SKILL.md` — expand Clarify Brief template, replace "Prefer assumptions over investigation" language, wire Validate against per-task success criteria. Patch version bump v0.19.0 → v0.19.1 via `python scripts/bump-version.py patch`. No source-code changes, no agent-file edits, no other skill touched.
- **Author:** Tasanakorn (design) + Claude Code (PRD authoring)

---

## 1. Goals

1. **Make silent assumptions impossible.** Both skills' Clarify Brief templates gain a required `Assumptions:` field. The consultant (st-lite) and the in-conversation Clarify flow (st-prd) must list the assumptions they are making rather than acting on them silently. Directly addresses Karpathy P1 ("state assumptions explicitly") for the two skills the user is routing most of their speculative work through.
2. **Make validation goal-driven.** The st-lite Brief gains a required `Success criteria:` field. The Validate phase override changes from a generic build/smoke check to a check against those criteria. Karpathy P4 ("transform tasks into verifiable goals; strong criteria enable independent iteration"). Current `st-lite:Validate` (lines 88–99) is a generic pass/fail; after this PRD it is anchored to the brief.
3. **Remove the direct conflict in `st-lite` Clarify.** The line `Prefer assumptions over investigation` (current `st-lite/SKILL.md:53`) is replaced with `State assumptions explicitly in the Assumptions: field; don't investigate to remove them.` The zero-pause fast-feedback property is preserved — assumptions are stated, not queried — but the letter of Karpathy P1 no longer conflicts with the skill's prose.
4. **Make `st-prd` fully Karpathy-aligned across all four principles.** Author override gains (a) a no-speculative-design clause (P2) permitting — and requiring — section omission when scope has nothing real to put under Migration or Edge Cases; (b) an "Alternatives considered" mandate (P1 tradeoffs + P2 simpler-approaches) requiring one short line per major design choice; (c) a style-match mandate (P3) requiring the authored PRD to match prose voice, heading depth, and admonition conventions of the most recent 2–3 PRDs in `docs/prd/`; (d) a Karpathy-format mandate for the Testing section — each acceptance step written as `step → verify:` (P4) while keeping the top-level heading name unchanged.
5. **Upgrade `st-prd` Clarify for multiple interpretations.** Current rules (lines 56–59) already ask-rather-than-guess for vague requests. Extend: if the user's request has two or more plausible framings, the Clarify dialogue must present both in the brief step and ask which — never pick silently. Karpathy P1 ("present multiple interpretations when they exist"). No pause regression because Clarify is already interactive.
6. **Lock-step patch version bump** to `v0.19.1` via `python scripts/bump-version.py patch`, propagating to `apps/stele/Cargo.toml`, `plugins/stele/.claude-plugin/plugin.json`, `plugins/steop/.claude-plugin/plugin.json`, and `apps/steop/version.go`. Patch is the correct tier under `docs/versioning.md` — this is additive skill-prose guidance, not a new feature or breaking contract.

## 2. Non-goals

- **No changes to `/steop:st-flow`.** The same Karpathy gaps (no Execute YAGNI override, no surgical clause, Plan phase auto-accept) are real but out of scope here. Flag-and-defer; a separate PRD can take them if desired.
- **No agent-file edits.** `plugins/steop/agents/consultant.md`, `architect.md`, `reviewer.md`, `executor.md`, `researcher.md` are untouched. All behavior changes ride via the per-phase override instructions already injected by the two target skills, consistent with how `/steop:st-flow` and `/steop:st-lite` have always steered their agents.
- **No Go or Rust source changes.** `apps/steop/cmd_state.go`, `apps/steop/cmd_statusline.go`, `apps/steop/internal/store/state.go`, and the entire `apps/stele` crate tree are untouched. The `--mode prd` / `--mode lite` tags keep their opaque-string semantics; no value is added or removed.
- **No retroactive edits to existing PRDs.** The `docs/prd/` directory is touched only for the new PRD-025 file. Prior PRDs — including the PRD-024 `**Status:** Proposed` drift flagged in conversation before this PRD was authored — are left alone. One concern per PRD.
- **No pauses or approval gates reintroduced.** `st-lite`'s zero-pause property (rules 1–2 at lines 14–16) is preserved intact. `st-prd`'s interactive Clarify is already gated; no new gates are added there either.
- **No reshaping of the PRD section template's top-level headings.** The canonical order — Goals, Non-goals, Background & Motivation, Design, Changes by Component, Edge Cases, Migration, Testing — stays. Karpathy P4 is enforced via the *content* of the Testing section (step → verify format), not by renaming it to Acceptance. This keeps the style-match mandate in Goal 4c internally consistent: the first PRD produced after this ships will look stylistically identical to PRDs 020–024 at the heading level.
- **No backfill of `Assumptions:` or `Success criteria:` fields** in historically captured briefs. The two skills do not persist their briefs anywhere, so there is nothing to backfill. The new fields take effect on the next invocation of each skill.

## 3. Background & Motivation

### 3.1 Current state

Both `/steop:st-prd` (Implemented v0.9.1, [PRD-004](prd-004-st-prd-skill.md)) and `/steop:st-lite` (Implemented v0.14.0 under its original `st-xp` name per [PRD-018](prd-018-st-xp-skill.md), renamed by [PRD-024](prd-024-rename-st-xp-to-st-lite.md) in v0.19.0) are working, shipped skills. Their current Clarify Brief shapes are:

```
# st-prd (plugins/steop/skills/st-prd/SKILL.md:36–42)
Title:      <working title>
Scope:      <affected components — 1 line>
Goals:      <1–3 bullets>
Non-goals:  <1–3 bullets>
Version:    <vX.Y.Z>

# st-lite (plugins/steop/skills/st-lite/SKILL.md:57–62)
Objective:   <one line>
Approach:    high | ambiguous
Complexity:  trivial | moderate | complex
Groups:      [G1: files/area, G2: ..., G3: ...]   # only if independent
```

Neither brief has a slot for assumptions or for task-specific success criteria. `st-lite`'s Validate override (lines 94–96) checks a fixed checklist — "does it build? does the main path run? are there obvious regressions?" — independent of what the work was. `st-prd`'s Author override (lines 87–96) prescribes a mandatory 8-section template without guidance on omitting sections when a PRD has nothing real to put in them.

[Andrej Karpathy's CLAUDE.md guide](https://github.com/forrestchang/andrej-karpathy-skills/blob/main/CLAUDE.md) (~55k GitHub stars as of April 2026) distills four principles into a single behavioral file:

1. Think Before Coding — state assumptions explicitly, surface tradeoffs, present multiple interpretations.
2. Simplicity First — minimum code solving the stated problem; no speculative features or abstractions.
3. Surgical Changes — match existing style, touch only what the task requires.
4. Goal-Driven Execution — verifiable success criteria, loop until verified.

Conversation with the user mapped the overlap: `st-prd` is already strong on P1 (interactive Clarify) and P3 (surgical by design) but has gaps on P2 (speculative sections forced) and P4 (no per-PRD success criteria). `st-lite` is strong on P2 + P3 (explicit YAGNI + no-refactor in its Execute override, line 76) but has a direct P1 conflict (`Prefer assumptions over investigation`, line 53) and the same P4 gap as `st-prd`.

### 3.2 Why now

- **Low implementation cost, high alignment return.** Every change in this PRD is prose-level inside two `SKILL.md` files. No Go, no Rust, no agent files, no hook JSON, no marketplace manifests. The patch bump ships without touching any code path.
- **The skills are in active use.** Both skills were exercised during the conversation that drove this PRD — the user reached for `/steop:st-prd` to author this very document. Fixing the briefs while the user is actively driving work through them produces immediate feedback on whether the new fields improve outcomes.
- **The Karpathy file itself is a living, externally maintained artifact.** The skills can't depend on it directly (it's an external repo), but picking up its four principles as internal convention aligns the two most complex workflow skills in the plugin with a widely adopted external baseline.
- **`st-lite` line 53 is the only direct prose conflict in the entire `plugins/steop/skills/` tree** with the Karpathy guide. Resolving it is a single-line rewrite. Leaving it resolved removes a perpetual footgun for future skill edits that copy-paste from `st-lite` as a template.

## 4. Design

### 4.1 `st-lite` Clarify — Brief template expansion

The Brief output shape currently has four fields. It gains two more. Position matters: `Assumptions` goes directly after `Objective` (so the assumptions are visible *before* the approach is committed); `Success criteria` goes directly before `Groups` (so it's read as part of the "what counts as done" frame, not as an afterthought).

Target shape after this PRD:

```
Objective:         <one line>
Assumptions:       <0–3 bullets — list explicitly; do NOT investigate to remove>
Approach:          high | ambiguous
Complexity:        trivial | moderate | complex
Success criteria:  <1–3 bullets — verifiable; each is independently checkable>
Groups:            [G1: files/area, G2: ..., G3: ...]   # only if independent
```

**Zero-bullet `Assumptions` is allowed and expected** for trivial tasks (pure renames, single-flag toggles) where no assumption is load-bearing. The field is *required* as a line; its content can be `(none)`. This forces the consultant agent to pause for a beat and produce the answer "no assumptions" or the actual list, rather than skipping the step.

**`Success criteria` for a `complexity: trivial` task** may be a single bullet, e.g. `file X at line Y reads <new string>`. For `complexity: moderate` or `complex`, at least one criterion must reference a *behavioral* outcome (build passes, feature runs, test passes) rather than only a textual outcome.

### 4.2 `st-lite` Clarify — override rewrite (line 53)

Current prose:

> **LITE MODE:** Produce a minimal brief in 1–3 tool calls max. Do NOT ask questions unless the request is genuinely ambiguous. Emit only: 1-line objective, approach confidence (`high` or `ambiguous`), complexity guess (`trivial`/`moderate`/`complex`), and optional `Groups:` list when the work splits into disjoint file sets. If complexity=`complex`, append a one-line suggestion to consider `/steop:st-flow` instead — but still proceed. Prefer assumptions over investigation.

Replacement prose:

> **LITE MODE:** Produce a minimal brief in 1–3 tool calls max. Do NOT ask questions unless the request is genuinely ambiguous. Emit the full brief shape: 1-line objective, explicit `Assumptions:` list (zero or more bullets — if none, write `(none)`), approach confidence (`high` or `ambiguous`), complexity guess (`trivial`/`moderate`/`complex`), `Success criteria:` (1–3 verifiable bullets), and optional `Groups:` list when the work splits into disjoint file sets. If complexity=`complex`, append a one-line suggestion to consider `/steop:st-flow` instead — but still proceed. **State assumptions explicitly in the `Assumptions:` field; do NOT investigate to remove them.**

The final sentence is the Karpathy P1 reconciliation. It preserves the speed bias (no investigation) while making the assumptions visible (they land in the brief). The `(none)` escape valve keeps trivial tasks frictionless.

### 4.3 `st-lite` Validate — override rewrite (lines 94–96)

Current prose:

> **LITE MODE:** Lightweight smoke check only. Does it build? Does the main path run? Are there obvious regressions in touched files? Do NOT audit exhaustively, do NOT run full test suites unless the project has a fast `make check` or equivalent. Report `pass` or `fail` with a one-line reason.

Replacement prose:

> **LITE MODE:** Check each bullet in the `Success criteria:` section of the Clarify Brief. A criterion is satisfied when you can *observe* it — run the command, read the file, open the page. Also smoke-check the generics: does it build? does the main path run? are there obvious regressions in touched files? Do NOT audit exhaustively, do NOT run full test suites unless the project has a fast `make check` or equivalent. Report `pass` or `fail` with a one-line reason that references the criterion it passed or the first one it failed.

Validate is now anchored to the brief. A `pass` without criterion-by-criterion observation is not a `pass`.

### 4.4 `st-prd` Clarify — Brief template expansion (lines 36–42)

The current template is five fields. It gains two more in the same position logic as `st-lite`:

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

**`Success criteria` semantics for `st-prd` are dual-horizon** — the criteria describe both (a) what makes the authored document correct at merge time (e.g. "docs/README.md row exists", "number allocation is unique"), and (b) what would make the eventual implementation successful (e.g. "skill invocation shows new brief shape on next run"). The consultant/Clarify flow does not pick between horizons; it lists whichever is load-bearing.

### 4.5 `st-prd` Clarify — rules upgrade (lines 56–59)

Current rules list three items (never skip, keep lightweight, ask on vague). A fourth is added explicitly for the multiple-interpretations case:

> - **Present alternatives when the request has two or more plausible framings.** If a user says "apply Karpathy to the skills," and that could mean (a) rewrite a single skill, (b) edit multiple skills' prose, (c) add a new cross-cutting skill that enforces Karpathy principles — present all three framings in the Brief step and ask which. Do not pick silently. This is different from "vague" (which Rule 3 already handles) — the request is concrete, but multiple concrete interpretations satisfy it.

### 4.6 `st-prd` Author — override rewrite (lines 85–96)

The current override lists six bullet conventions. Four are preserved; the `Section template` and `Status` bullets are reshaped, and three new bullets are added. New full form:

> **PRD MODE:** You are writing a PRD document, not an implementation blueprint. Use the PRD Brief and Research Summary to produce a complete PRD file. Follow these conventions exactly:
>
> - **Number allocation:** Scan `docs/prd/` for the highest `NNN` in filenames matching `prd-NNN-*.md`, increment by 1, zero-pad to 3 digits.
> - **Filename:** `prd-NNN-<slug>.md` where slug is derived from the title in kebab-case, max 5 words.
> - **Author field:** Run `git config user.name` to get the name, then format as `<name> (design) + Claude Code (PRD authoring)`.
> - **Section template (canonical order — top-level headings unchanged):** Goals, Non-goals, Background & Motivation (with "Current state" subsection), Design, Changes by Component (table), Edge Cases, Migration, Testing. **Omit sections with nothing real to put in them** — if the PRD has no migration, drop the `## 7. Migration` heading rather than writing "None." If the PRD has no non-trivial edge cases, drop `## 6. Edge Cases`. Never write placeholder prose for the sake of structural completeness.
> - **Testing section format:** Each testable outcome written as `step → verify:` — a concrete action on the left, an observable signal on the right. This is Karpathy's P4 verifiable-goals convention lifted into the document body. The top-level heading stays `## N. Testing` for style continuity with PRDs 020–024.
> - **Alternatives considered:** For each major design choice, include one short line — inline in the relevant `### 4.x` subsection — noting a simpler alternative considered and why it was rejected. Format: `**Alternative considered:** <one line>. Rejected: <one line>.` Do *not* add a standalone `## Alternatives` section; a single summary section is too coarse and breaks surgical-change flow.
> - **Style match:** Read the most recent 2–3 PRDs in `docs/prd/` (sorted by filename) before you write. Match their prose voice (declarative, not hortative), heading depth (top-level `##`, subsection `###`, rarely `####`), admonition conventions (blockquote `> **Superseded by ...**` when applicable), and table column styles. The PRD you write should be visually indistinguishable from its neighbors.
> - **Status:** `Proposed` unless the user specified otherwise during Clarify.
> - **README update:** Add a row to the PRD table in `docs/README.md` with the new PRD link, status, and one-line description. Do NOT touch adjacent rows, even to fix known drift. One concern per PRD.
>
> After writing, display the file path and a one-line summary. Do NOT ask for approval — the user will review and edit manually.

### 4.7 Version bump

`python scripts/bump-version.py patch` lifts v0.19.0 → v0.19.1 across four targets in lock-step:

- `apps/stele/Cargo.toml` (workspace version — pulls `apps/stele/Cargo.lock` forward automatically via `cargo update --workspace`).
- `plugins/stele/.claude-plugin/plugin.json`.
- `plugins/steop/.claude-plugin/plugin.json`.
- `apps/steop/version.go`.

A skill-prose-only change is a **patch** under `docs/versioning.md` ("fixes and docs"). The briefs the skills emit change shape, which is a soft contract shift, but no downstream consumer (no hook, no agent, no CLI flag, no MCP tool) reads those briefs programmatically — they are freeform prose passed between phases in the same conversation. Patch is the honest tier.

## 5. Changes by Component

| Component                         | Change                                                                                                                                                                                                 | Files                                                                                                                                       |
| --------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ | ------------------------------------------------------------------------------------------------------------------------------------------- |
| `st-lite` Brief template          | Expand the code-block shape per §4.1. Add `Assumptions:` and `Success criteria:` rows.                                                                                                                 | `plugins/steop/skills/st-lite/SKILL.md` (block at lines 57–62 pre-edit)                                                                     |
| `st-lite` Clarify override        | Replace the LITE MODE prose block per §4.2. Swap "Prefer assumptions over investigation" for the explicit-state sentence.                                                                              | `plugins/steop/skills/st-lite/SKILL.md` (blockquote at line 53 pre-edit)                                                                    |
| `st-lite` Validate override       | Replace the LITE MODE prose block per §4.3. Anchor the check against `Success criteria:`.                                                                                                              | `plugins/steop/skills/st-lite/SKILL.md` (blockquote at lines 94–96 pre-edit)                                                                |
| `st-prd` Brief template           | Expand the code-block shape per §4.4. Add `Assumptions:` and `Success criteria:` rows.                                                                                                                 | `plugins/steop/skills/st-prd/SKILL.md` (block at lines 36–42 pre-edit)                                                                      |
| `st-prd` Clarify rules            | Append the multiple-interpretations rule per §4.5.                                                                                                                                                     | `plugins/steop/skills/st-prd/SKILL.md` (bullet list at lines 56–59 pre-edit)                                                                |
| `st-prd` Author override          | Rewrite the PRD MODE blockquote per §4.6. Preserve four existing bullets; reshape Section template; add Testing-format, Alternatives-considered, Style-match bullets.                                  | `plugins/steop/skills/st-prd/SKILL.md` (blockquote at lines 85–96 pre-edit)                                                                 |
| Workspace docs index (insert row) | Add PRD-025 row to the PRD table, ordered numerically after PRD-024. Do NOT touch adjacent rows.                                                                                                       | `docs/README.md` PRD table                                                                                                                  |
| Version bump                      | Run `python scripts/bump-version.py patch`; verify all four targets moved to `0.19.1`.                                                                                                                 | `apps/stele/Cargo.toml`, `plugins/stele/.claude-plugin/plugin.json`, `plugins/steop/.claude-plugin/plugin.json`, `apps/steop/version.go`    |

No source-code changes. No edits to `apps/steop/cmd_*.go`, `apps/steop/internal/**`, any `apps/stele/crates/**`, any agent in `plugins/steop/agents/`, any hook in `plugins/steop/hooks/`, any marketplace manifest at `.claude-plugin/`, or any other skill under `plugins/steop/skills/`.

**Alternative considered:** Rename the PRD template's `## 8. Testing` section to `## 8. Acceptance` to match Karpathy's P4 vocabulary directly. **Rejected:** the Style-match bullet in §4.6 requires the authored PRD to look like PRDs 020–024, which all use `## 8. Testing`. The first PRD produced after this ships would be self-contradictory (renaming the section while mandating style-match). Enforcing P4 via the *content* of the section, not its name, avoids the collision.

## 6. Edge Cases

| Scenario                                                                                | Behavior                                                                                                                                                                                                                                                                                                                                                                    |
| --------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| User invokes `/steop:st-lite` with a one-word request ("refactor")                       | **Genuine ambiguity → pause per existing rule 2 (line 16).** Nothing in this PRD changes that gate. After the user disambiguates, the consultant produces the new-shape brief with populated `Assumptions:` and `Success criteria:`.                                                                                                                                        |
| User invokes `/steop:st-lite` with a trivial rename task                                 | **Zero-bullet `Assumptions:` is expected.** The consultant writes `Assumptions: (none)` and proceeds. `Success criteria:` may be a single textual-outcome bullet. Zero friction added vs pre-PRD behavior.                                                                                                                                                                   |
| An existing consultant-agent prompt (elsewhere in the plugin) still says "prefer assumptions" | **Out of scope.** The change is skill-local. `plugins/steop/agents/consultant.md` is not edited by this PRD — the Clarify override in each skill is what actually steers the consultant per-invocation. If a future PRD wants to propagate the framing into the agent file itself, that is a separate effort.                                                                |
| `st-lite` Validate receives a brief with no `Success criteria:` (legacy-format brief)    | **Falls back to the generic smoke check.** The Validate override text reads "Check each bullet in the `Success criteria:` section of the Clarify Brief" — if that section is empty or absent, there are zero bullets to check and the generics apply unchanged. No error behavior needed.                                                                                    |
| `st-prd` Author phase cannot find 2–3 recent PRDs for style-match (empty `docs/prd/`)    | **Cannot happen in practice.** `docs/prd/` already contains PRDs 001–024. If a hypothetical future fork strips the PRDs, the architect falls back to its own judgment — the style-match mandate is soft guidance, not a hard precondition.                                                                                                                                   |
| User invokes `st-prd` with a request like "apply Karpathy to skills"                     | **Multi-framing Clarify fires per §4.5.** Present the three framings (single skill / multi skill / new enforcement skill) and ask which. This is the exact shape that drove this PRD's own Clarify dialogue — dogfooded by the authoring flow.                                                                                                                               |
| `st-prd` Author omits the Migration section for a no-migration PRD                        | **Allowed and encouraged per §4.6.** The canonical section list describes the default order, not a required minimum. Omitting is the Karpathy P2-aligned choice when the section would otherwise read "None." or "N/A."                                                                                                                                                      |
| An in-flight `/steop:st-lite` session started pre-upgrade, continues post-upgrade        | **No active-session remediation needed.** Neither skill persists briefs across conversation boundaries; each invocation regenerates its brief from scratch. The new fields appear on the next invocation.                                                                                                                                                                     |
| Downstream consumer (hook, RPC, script) programmatically parses the Brief               | **Does not exist.** Research confirmed the briefs are freeform prose consumed only by subsequent phases in the same conversation. No CLI or hook reads them as structured data. Changing the shape is therefore a patch, not a minor.                                                                                                                                         |

## 7. Migration

**Non-breaking.** Every change in this PRD is additive at the user-facing surface:

- Existing `/steop:st-lite` invocations still work. The Clarify override now produces a richer brief; downstream phases consume it verbatim. No call-site change.
- Existing `/steop:st-prd` invocations still work. The Clarify flow asks one extra thing (multi-framing check) only when the user's request has multiple plausible framings — concrete-single-framing requests see zero change.
- The `--mode prd` and `--mode lite` CLI tags keep their opaque-string semantics. No `steop` binary or Go-side change.
- The PRD section template stays structurally identical at the heading level. PRDs authored before this PRD ships and PRDs authored after are visually continuous.

**Version bump:**

```bash
cd /path/to/stele
python scripts/bump-version.py patch
# Verify:
grep -E '"?version"?\s*[:=]\s*"0\.19\.1"' \
  apps/stele/Cargo.toml \
  plugins/stele/.claude-plugin/plugin.json \
  plugins/steop/.claude-plugin/plugin.json
grep '"0.19.1"' apps/steop/version.go
```

**PRD-024 status drift (flag-and-defer).** Conversation before this PRD was authored noted that `docs/prd/prd-024-rename-st-xp-to-st-lite.md:3` and `docs/README.md:48` both still read `Proposed` despite v0.19.0 shipping the rename. That is a genuine drift and should be fixed — but by a separate edit, not inside this PRD's commit. One concern per PRD (Karpathy P3 surgical-change applied to the authoring flow itself).

**Flip the top-of-file `**Status:**`** on this PRD from `Proposed` to `Implemented (v0.19.1)` once §8 acceptance holds, and update the `docs/README.md` row for PRD-025 in the same commit.

## 8. Testing

No automated test harness is added — the skills are prose, and prose can only be exercised by live invocation. Manual acceptance sequence, each step written in Karpathy `step → verify:` format per Goal 4d (dogfooded here):

### 8.1 File-system and version

- Run `ls plugins/steop/skills/st-lite/SKILL.md plugins/steop/skills/st-prd/SKILL.md` → **verify:** both files exist.
- Run `python scripts/bump-version.py --list` → **verify:** workspace, stele, steop all read `0.19.1`.

### 8.2 `st-lite` Brief-shape sweep

- Run `grep -A 8 '^```$' plugins/steop/skills/st-lite/SKILL.md | grep -E 'Objective|Assumptions|Approach|Complexity|Success criteria|Groups'` → **verify:** six matches in the order `Objective`, `Assumptions`, `Approach`, `Complexity`, `Success criteria`, `Groups`.
- Run `grep -c 'Prefer assumptions over investigation' plugins/steop/skills/st-lite/SKILL.md` → **verify:** `0`.
- Run `grep -c 'State assumptions explicitly' plugins/steop/skills/st-lite/SKILL.md` → **verify:** `1`.

### 8.3 `st-lite` Validate-anchor sweep

- Run `grep -c 'Success criteria' plugins/steop/skills/st-lite/SKILL.md` → **verify:** at least `2` (one in the brief shape, one in the Validate override).
- Read the Validate override paragraph (pre-edit lines 94–96) → **verify:** prose references `Success criteria:` and retains the build/main-path generics as a secondary check.

### 8.4 `st-prd` Brief-shape sweep

- Read the Brief code block in `plugins/steop/skills/st-prd/SKILL.md` (pre-edit lines 36–42) → **verify:** seven fields in the order `Title`, `Scope`, `Assumptions`, `Goals`, `Non-goals`, `Success criteria`, `Version`.

### 8.5 `st-prd` Author-override sweep

- Run `grep -c 'Style match' plugins/steop/skills/st-prd/SKILL.md` → **verify:** at least `1`.
- Run `grep -c 'Alternatives considered' plugins/steop/skills/st-prd/SKILL.md` → **verify:** at least `1`.
- Run `grep -c 'Omit sections with nothing real' plugins/steop/skills/st-prd/SKILL.md` → **verify:** `1`.
- Run `grep -c 'step → verify' plugins/steop/skills/st-prd/SKILL.md` → **verify:** at least `1`.
- Run `grep -c '## 8. Acceptance' plugins/steop/skills/st-prd/SKILL.md` → **verify:** `0` (template heading was NOT renamed).

### 8.6 Live `st-lite` invocation

- In Claude Code, run `/steop:st-lite Add a trailing newline to README.md` → **verify:** Clarify output contains both `Assumptions:` and `Success criteria:` fields (even if `(none)` / single bullet respectively). Statusline shows `[lite] Clarify: …` → `[lite] Execute: …` → `[lite] Validate: pass` with a reason that references a criterion.

### 8.7 Live `st-prd` invocation

- In Claude Code, run `/steop:st-prd add a throwaway test PRD about foo` → **verify:** Clarify dialogue presents a brief containing `Assumptions:` and `Success criteria:`. If the request has two plausible framings (e.g. "foo" could be interpreted two ways), → **verify:** the dialogue presents both framings and asks rather than picking. Abort the dialogue before Author fires (this is a shape check, not an actual authored PRD).

### 8.8 Produce a dogfood PRD

- Run `/steop:st-prd` on any small real topic that genuinely has no Migration section → **verify:** authored file at `docs/prd/prd-NNN-*.md` omits the `## 7. Migration` heading entirely rather than writing "None." Check the file with `grep -c '## 7. Migration' docs/prd/prd-NNN-*.md` → **verify:** `0`.

### 8.9 Style-match sanity

- `diff` the heading structure (`grep '^##' docs/prd/prd-025-*.md` vs `grep '^##' docs/prd/prd-024-*.md`) → **verify:** same top-level section names (Goals, Non-goals, Background & Motivation, Design, Changes by Component, Edge Cases, Migration, Testing). PRD-025 may omit sections per §4.6; existing sections must match PRD-024's headings verbatim.

### 8.10 Rust and Go build (unchanged)

- Run `cd apps/stele && cargo build -p stele-server -p stele-cli` → **verify:** clean build. No Rust source touched.
- Run `cd apps/steop && go build -o target/steop .` → **verify:** clean build. No Go source touched.

## 9. Open Questions

1. **Should the `Success criteria:` field eventually propagate to `/steop:st-flow`?** The Flow skill has the same P4 gap (Validate phase runs on reviewer-agent heuristics rather than a pre-stated criterion). A follow-up PRD could carry the field from Clarify through Plan to Validate in the full pipeline. Out of scope here; the user explicitly scoped this PRD to `st-prd` + `st-lite`.
2. **Should the consultant and reviewer agent files eventually absorb the same language?** `plugins/steop/agents/consultant.md` and `plugins/steop/agents/reviewer.md` carry a default persona that is then overridden per-skill. The Karpathy framing in this PRD is baked into the skill-level overrides; moving it into the agent file would make it the default across every invocation (including future skills that don't yet exist). Deferred — the per-skill override is the surgical location for now.
3. **Should `st-prd` eventually grow a machine-readable brief format?** If downstream tooling ever needs to parse briefs (e.g., a linter that checks every PRD has an `Assumptions:` block), the current freeform-prose brief becomes a liability. Not a problem today. If it becomes one, a JSON or YAML brief can be added as a non-breaking alternative.
