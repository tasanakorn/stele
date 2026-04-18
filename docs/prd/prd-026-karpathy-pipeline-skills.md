# PRD-026 — Karpathy alignment for `/steop:st-flow` and the standalone phase skills

- **Status:** Implemented (v0.19.2)
- **Target version:** v0.19.2
- **Scope:** `plugins/steop/skills/st-flow/SKILL.md` — drop two P1 conflicts, make Task Brief shape explicit, add three new phase-override blocks (Research, Execute, Validate) that do not exist today, anchor Validate against `Success criteria`. `plugins/steop/skills/{st-clarify, st-research, st-plan, st-execute, st-validate}/SKILL.md` — align each standalone skill's Brief/Summary output shape and override prose with the Karpathy pattern established by PRD-025. Patch version bump v0.19.1 → v0.19.2 via `python scripts/bump-version.py patch`. No source-code changes, no agent-file edits.
- **Author:** Tasanakorn (design) + Claude Code (PRD authoring)

---

## 1. Goals

1. **Remove `/steop:st-flow`'s two hard P1 conflicts.** The Clarify override at `st-flow/SKILL.md:59` (`Prefer making reasonable assumptions over asking questions`) and the Plan override at `:99` (`Do NOT present it for approval or ask for adjustments`) both directly contradict Karpathy P1 ("State assumptions explicitly. If uncertain, ask."). Both are rewritten to preserve zero-pause semantics while making assumptions and alternatives *stated*, not silent. Mirrors the reconciliation shipped for `st-lite` in PRD-025.
2. **Make `/steop:st-flow`'s Task Brief shape explicit.** Today the brief is implicit — the consultant agent produces it from its own agent-file template and st-flow never shows the expected shape. Add an explicit code block with `Objective / Assumptions / Success criteria / Complexity / Groups` (close to st-lite's shape but pipeline-flavored). This is a forcing function: the consultant cannot omit a field that is visible in the skill prose.
3. **Add three new phase-override blocks to `/steop:st-flow`** — Research, Execute, and Validate. None of these exist today. The skill currently does not inject any override into those three agents; they run on their agent-file defaults. PRD-026 adds a short `> **FLOW MODE:**` blockquote to each, carrying the Karpathy framing: Research surfaces assumptions-made, Execute runs YAGNI + surgical-change, Validate anchors against the brief's `Success criteria`. These are additive skill-prose blocks; no agent-file is touched.
4. **Align the five standalone phase skills** (`/steop:st-clarify`, `/steop:st-research`, `/steop:st-plan`, `/steop:st-execute`, `/steop:st-validate`) with the same Karpathy pattern — Assumptions + Success criteria in Brief/Summary outputs, YAGNI + surgical overrides in Execute, Alternatives-considered in Plan, criterion-anchored Validate. These edits are **orthogonal** to the st-flow edits: each standalone skill is invoked independently of st-flow (st-flow uses agents directly, not the standalone skills). Alignment is done for coherence across the plugin surface, not because st-flow depends on the standalones.
5. **Lock-step patch version bump** to `v0.19.2` via `python scripts/bump-version.py patch`, propagating to `apps/stele/Cargo.toml`, `plugins/stele/.claude-plugin/plugin.json`, `plugins/steop/.claude-plugin/plugin.json`, and `apps/steop/version.go`. Patch tier under `docs/versioning.md` ("fixes and docs") — this is additive skill-prose guidance, not a new feature or breaking contract.

## 2. Non-goals

- **No agent-file edits.** `plugins/steop/agents/{consultant, researcher, architect, executor, reviewer}.md` stay untouched. All behavior changes ride via the per-skill override blocks, consistent with PRD-025's treatment of st-lite/st-prd and with PRD-025 Open Q #2's explicit deferral of agent-file work.
- **No Go or Rust source changes.** `apps/steop/cmd_*.go`, `apps/steop/internal/**`, and the entire `apps/stele` tree are untouched. The `--mode flow`, `--mode clarify`, `--mode research`, etc. tags keep their opaque-string semantics.
- **No changes to `/steop:st-send`, `/steop:st-watch`, `/steop:install`, `/steop:statusline-setup`.** These are task routers and utilities, not code-authoring workflows. They have no meaningful Karpathy-aligned surface to edit (no Brief, no Validate, no Execute semantics that map to the four principles).
- **No changes to `/steop:st-prd` or `/steop:st-lite`.** Both were fully aligned in PRD-025 (v0.19.1). PRD-026 does not touch them — not even to fix unrelated drift.
- **No retroactive edits to prior PRDs.** `docs/prd/` is touched only for the new PRD-026 file plus one PRD-026 row added to `docs/README.md`. One concern per PRD.
- **No reshaping of st-flow's phase structure.** The five phases (Clarify → Research → Plan → Execute → Validate), the ambiguity gate, the retry loop (up to 3 rounds on validation failure), the complexity-based model selection, and the stop conditions all stay exactly as in v0.19.1. Karpathy alignment is layered on top of the existing structure, not a redesign of it.
- **No new Task Brief shape for the consultant agent file.** The explicit code block added to `st-flow/SKILL.md` (Goal 2) is scoped to the skill, not to the agent's own instructions. The consultant agent's own default template remains as-is; the st-flow invocation overrides it inline.

## 3. Background & Motivation

### 3.1 Current state

After PRD-025 (v0.19.1, Implemented), `/steop:st-prd` and `/steop:st-lite` are fully Karpathy-aligned across the four principles (Think Before Coding, Simplicity First, Surgical Changes, Goal-Driven Execution). Ten other skills ship in `plugins/steop/skills/`: `st-flow`, `st-clarify`, `st-research`, `st-plan`, `st-execute`, `st-validate`, `st-send`, `st-watch`, `install`, `statusline-setup`. Of these, the eight non-utility skills divide cleanly:

- **`st-flow`** — the full-pipeline orchestrator. Has explicit override blocks only at Clarify (`:59`) and Plan (`:99`), both of which directly contradict Karpathy P1. Has no override blocks for Research (~L71–87), Execute (~L105–120), or Validate (~L122–131) — those phases delegate entirely to the respective agents' defaults. Task Brief shape is implicit: the consultant agent is "asked to produce a Task Brief" but the shape is not written anywhere in the skill.
- **5 standalone phase skills** — `st-clarify`, `st-research`, `st-plan`, `st-execute`, `st-validate`. Each is invocable as its own slash command. They share the same five agents as st-flow but run without st-flow's orchestration. Their Brief/Summary outputs are documented inline in each skill. `st-clarify` already ships an `Assumptions` field (`:44`) and an `Open questions` field (`:45`) — the closest-to-Karpathy of any skill in the plugin pre-PRD-025. The other four have simpler Summary shapes with no Assumptions or Success criteria slot.
- **`st-send`, `st-watch`** — task-routing utilities. They move work between sessions but do not produce Briefs or perform Validates themselves. No Karpathy-relevant surface.
- **`install`, `statusline-setup`** — one-shot installation and configuration helpers. Same: no Karpathy-relevant surface.

### 3.2 The orthogonality that shaped the Design section

A deliberate architectural fact — caught during this PRD's Research phase and load-bearing for §4 — is that **st-flow invokes the five agents directly, not the five standalone phase skills.** The skills and the flow are two parallel surfaces over the same agent pool. Consequences:

- Edits to `st-validate/SKILL.md` do *not* change how st-flow validates, because st-flow's Phase 5 block passes no override to the reviewer agent (today — PRD-026 changes that by adding one). The reviewer, when invoked by st-flow, reads the agent-file default, not the standalone skill.
- Equivalently, edits to `st-flow/SKILL.md`'s Validate override do *not* change how a bare `/steop:st-validate` invocation behaves.
- The two surfaces are orthogonal. PRD-026 treats them as such: §4.1–4.4 cover st-flow, §4.5–4.9 cover the standalones independently.

### 3.3 Why now

- **PRD-025 (v0.19.1) just shipped** and landed cleanly — a minute after implementation the user re-audited both skills against the Karpathy guide and the gap count dropped from 9 to 1 (the last residual drift was patched in commit `602ba36`). The pattern is fresh, the template files are small, and extending it to the remaining pipeline surfaces is low-risk and high-consistency-return.
- **The two P1 conflicts in st-flow are the highest-visibility Karpathy violations in the entire plugin.** `/steop:st-flow` is the default pipeline and gets invoked most often. Leaving those two lines in-place while calling the smaller skills "fully aligned" creates a perception gap: users see the new Brief shape in `st-lite` but don't see it in `st-flow`, the more commonly-used sibling.
- **The three missing Execute/Research/Validate override blocks in st-flow are a silent defect.** Today, the reviewer agent invoked by st-flow runs on its agent-file defaults — which don't include any Karpathy framing. This means st-flow's Validate phase is *weaker* than st-lite's (st-lite has an explicit override anchoring to `Success criteria:`; st-flow does not). A user invoking the full 5-phase pipeline should not receive *less* Karpathy-aligned review than a user invoking the compressed 3-phase pipeline. PRD-026 fixes this inversion.
- **Five standalone phase skills exist but are only loosely documented as a coherent surface.** Most users reach for `/steop:st-flow`; the standalones serve debugging, pedagogy, and one-off phase reruns. Aligning them keeps the plugin's story consistent without forcing users to pick between two semantics.

## 4. Design

### 4.1 `st-flow` Clarify override rewrite

Current prose at `plugins/steop/skills/st-flow/SKILL.md:59`:

> **FLOW MODE:** Do NOT ask clarifying questions or wait for user confirmation unless the request is genuinely ambiguous (no identifiable action, contradictory, or multiple incompatible interpretations). If the intent is clear enough to act on, produce the Task Brief and return immediately. Prefer making reasonable assumptions over asking questions.

Replacement prose:

> **FLOW MODE:** Do NOT ask clarifying questions or wait for user confirmation unless the request is genuinely ambiguous (no identifiable action, contradictory, or multiple incompatible interpretations). If the intent is clear enough to act on, produce the Task Brief per the shape below and return immediately. **State assumptions explicitly in the `Assumptions:` field of the brief; do NOT investigate to remove them.** If two or more concrete framings plausibly satisfy the request, list them under `Open questions:` rather than picking silently.

This preserves the zero-pause contract (no new gates) while closing the P1 conflict (assumptions stated, not silent) and adding the P1 multiple-interpretations hook (framings listed, not picked). Word-for-word reconciliation is identical to the PRD-025 rewrite that shipped for st-lite.

**Alternative considered:** Reintroduce an approval pause when assumptions are load-bearing. Rejected: st-flow's zero-pause property is its core value prop vs. bare agent chains; trading it for Karpathy perfection is not a patch-tier change.

### 4.2 `st-flow` Task Brief explicit shape

Immediately after the Clarify override block in §4.1, add a code block matching the skill prose pattern established by `st-lite/SKILL.md:57–64`:

```
Objective:         <one line>
Assumptions:       <0–3 bullets — list explicitly; do NOT investigate to remove>
Complexity:        simple | standard | complex
Success criteria:  <1–3 bullets — verifiable; each is independently checkable>
Open questions:    <0–N bullets — alternate framings the consultant surfaced>
Groups:            [G1: files/area, G2: ..., G3: ...]   # only if independent
```

**`Open questions` in FLOW MODE is informational-only.** It lists what the consultant chose not to investigate; it does not pause the pipeline. If the list is non-empty, the Finalize summary surfaces it at the end so the user sees what was assumed-away. Rules 1–2 (zero-pause + single ambiguity gate) are unchanged.

**Complexity** stays on the `simple/standard/complex` axis from the current implicit brief (used by Phase 2 to skip Research when `simple` and by Phase 4 to pick the executor model). `Complexity: trivial` from st-lite's shape is absent here — st-flow never downgrades below haiku on Simple.

**Alternative considered:** Reuse st-lite's six-field brief shape verbatim. Rejected: the `Approach:` field (`high | ambiguous`) is a st-lite-specific driver for parallel exploration; st-flow does not fan out on ambiguity, so the field would be dead prose.

### 4.3 `st-flow` Plan override rewrite

Current prose at `plugins/steop/skills/st-flow/SKILL.md:99`:

> **FLOW MODE:** Produce the implementation blueprint and return it. Do NOT present it for approval or ask for adjustments. The executor will follow it directly.

Replacement prose:

> **FLOW MODE:** Produce the implementation blueprint and return it. Do NOT pause for approval — the executor will follow it directly. For each major design choice, include an inline **Alternatives considered:** line (simpler option, one-line rationale for rejection). Prefer the simpler of two equivalent designs; if YAGNI points one way and the plan goes the other, state the reason.

This preserves zero-pause (no approval pause) but closes the P1 conflict (tradeoffs are surfaced inline, not hidden) and adds the P2 simpler-approaches hook. The "Do NOT present it for approval or ask for adjustments" line is softened to "Do NOT pause for approval" — same operational outcome (no wait), different framing (decisions are still stated in-line for the reader/executor).

**Alternative considered:** Add an explicit "Trade-offs" subsection to every Plan output. Rejected: st-plan already has "Architecture decisions — trade-offs considered and choices made" (`:28`); duplicating structure creates drift risk. Inline Alternatives-considered lines are the same-payload lower-weight form.

### 4.4 `st-flow` new override blocks (Research, Execute, Validate)

Three phase blocks in `st-flow/SKILL.md` currently contain only model-selection prose and a status-emit line — no `> **FLOW MODE:**` override. This PRD adds one block to each:

**Research (inserted after the model-selection bullets at L79–82):**

> **FLOW MODE:** List at the end of your Research Summary the assumptions you made about *which files to read* and *which areas to skip* — don't investigate away every uncertainty; state them. If a relevant-looking file was deprioritized due to time, note it under `Assumptions` so the architect can reconsider.

**Execute (inserted after the model-selection bullets at L111–116):**

> **FLOW MODE:** Implement the plan. Prefer YAGNI — skip defensive code, edge cases, and polish unless they block the happy path or are named in the plan. Leave TODOs where assumptions are load-bearing. Do NOT refactor neighboring code. Return as soon as the planned steps are complete and the main path works.

Mirrors `st-lite/SKILL.md:78` verbatim except for "the plan" substitution ("smallest working slice" → "the plan").

**Validate (inserted before the pass/fail emit rules at L130–131):**

> **FLOW MODE:** Check each bullet in the `Success criteria:` section of the Task Brief. A criterion is satisfied when you can *observe* it — run the command, read the file, open the page. Also: does the implementation match the Plan's steps? Are there obvious regressions in touched files? Report `Pass` or `Fail`. On `Fail`, name the first unsatisfied criterion (or regression) — the Execute-Validate retry loop (Rule 4) needs a concrete target to fix.

Anchors the reviewer agent to the Task Brief's criteria. The retry loop at Rule 4 now has a named target per cycle, which should reduce loop-waste.

**Alternative considered:** Edit the reviewer agent file (`plugins/steop/agents/reviewer.md`) to bake in the criterion-anchored check. Rejected per PRD-025 Open Q #2 deferral — agent-file edits have a wider blast radius (every skill using the reviewer inherits the change, including unborn skills). Skill-level overrides stay surgical.

### 4.5 `st-clarify` standalone alignment

Current Task Brief shape (`st-clarify/SKILL.md:40–45`):

```
- **Objective** — one-sentence statement
- **Scope** — explicit boundaries
- **Complexity** — simple / standard / complex
- **Assumptions** — anything assumed that wasn't explicitly stated
- **Open questions** — any remaining questions for the user (if none, state "None")
```

Post-PRD change: add one bullet between `Complexity` and `Assumptions`:

```
- **Success criteria** — 1–3 verifiable bullets; each independently checkable
```

Plus: append a new rule to any rules/instruction block present in the skill — at the bottom of Step 2 Clarify bullets (~L35):

- **Present alternatives when the request has two or more plausible framings.** If the user's concrete request admits multiple concrete interpretations, list them under **Open questions** and ask which — don't pick silently. Distinct from "unclear" (which is already covered by the existing "ask clarifying questions" guidance).

Multi-framing hook lives in Open questions so the field does double duty — unanswered questions and alternate framings both surface to the user.

**Alternative considered:** Drop `Open questions` and replace with a dedicated `Framings` slot. Rejected: `Open questions` is already used by consumers and removing it risks breaking downstream expectations; multi-framings fit inside the existing field's semantics.

### 4.6 `st-research` standalone alignment

Current Summary shape (`st-research/SKILL.md:37–42`):

```
- **Relevant files** — paths and roles
- **Patterns** — conventions and approaches
- **Dependencies** — what connects to what
- **Constraints** — things to watch out for
- **Key context** — code snippets or decisions
```

Post-PRD change: add one bullet at the end:

```
- **Assumptions** — areas deprioritized or not investigated; files the agent
  chose not to read and why (time, apparent irrelevance). State don't investigate.
```

Explicit Karpathy P1 applied to a read-only phase: even Research is allowed to skip things, but what it skipped is stated, not hidden. The architect consuming the summary knows what was assumed-away.

**Alternative considered:** Add an "Explored vs. skipped" binary list. Rejected: the `Assumptions` bullet is a prose field; a matrix would be heavier than necessary for the typical case (0–3 skipped areas).

### 4.7 `st-plan` standalone alignment

Current Plan output (`st-plan/SKILL.md:22–29`):

```
- **Goal**
- **Steps**
- **Architecture decisions** — trade-offs considered and choices made
- **Testing strategy**
```

Post-PRD change: reshape two bullets to make the Karpathy vocabulary explicit, and add one new guidance paragraph in the instruction body (after L20):

- `Architecture decisions` → keep the heading but add to its body: *"For each decision, state an **Alternative considered** (simpler option the decision rejected) in one line. Format: `<chosen> chosen over <alternative> because <reason>`."*
- `Testing strategy` → augment: *"Reference the Task Brief's **Success criteria** — don't restate them; point to them. Add plan-specific verification steps using `step → verify:` format where helpful."*

New guidance paragraph (inserted after L20):

> Prefer the simpler of two equivalent designs. If the Research Summary flagged an area that YAGNI would skip, skip it unless the Task Brief's Success criteria require it. Don't design for hypothetical future requirements not stated in the brief.

**Alternative considered:** Introduce a dedicated `## Alternatives` section in every Plan output. Rejected per PRD-025's own Author override rule (Alternatives inline, not as a section) — which this PRD must consistent-with since it ships after that rule took effect.

### 4.8 `st-execute` standalone alignment

Current Execution Goals (`st-execute/SKILL.md:30–36`):

```
- Follow the plan step by step
- Make all necessary code changes
- Keep changes focused and minimal — implement what was planned, nothing more
- Report what was changed after completion
```

Post-PRD change: replace the bullet list with a prose paragraph matching `st-lite/SKILL.md:78`'s explicit override framing, plus one surgical-change clause:

```
The execution agent(s) should follow the plan step by step and return as
soon as the planned steps are complete. Prefer YAGNI — skip defensive code,
edge cases, and polish unless they block the happy path or are named in
the plan. Leave TODOs where assumptions are load-bearing. Do NOT refactor
neighboring code. Only remove imports, variables, or functions that your
changes made unused. Every changed line should trace to the plan.
Report what was changed after completion.
```

The "Only remove imports/variables/functions that your changes made unused" and "Every changed line should trace to the plan" clauses are Karpathy P3 — surgical changes — lifted directly from the Karpathy guide's own wording. This is the only standalone skill edit that adds explicit P3 framing; st-lite gets it via "Do NOT refactor neighboring code" alone, which is sufficient for the compressed pipeline but not for the full one where plan steps can drift.

**Alternative considered:** Keep the bullet list and add new bullets. Rejected: the existing bullets blend with the new guidance awkwardly ("Make all necessary code changes" vs "Prefer YAGNI — skip edge cases"); a cohesive paragraph is clearer.

### 4.9 `st-validate` standalone alignment

Current instructions (`st-validate/SKILL.md:18–31`):

```
The verification agent should:
1. Review changes
2. Check correctness
3. Run tests
4. Check consistency
5. Check completeness

After the agent completes, present a verification report:
- Status — Pass / Fail / Issues Found
- Changes reviewed
- Issues
- Test results
- Recommendations
```

Post-PRD change: insert a step 0 before "Review changes":

```
0. **Check Success criteria first.** Read the Task Brief's `Success criteria:`
   bullets (if available). A criterion is satisfied when you can *observe* it —
   run the command, read the file, open the page. A missing or unobservable
   criterion is a `Fail`, not a `Pass`.
```

And update the report shape to add:

```
- Criterion results — bullet-per-criterion, observed outcome
```

**Alternative considered:** Make `Criterion results` the Status itself (drop Pass/Fail/Issues Found). Rejected: the Pass/Fail/Issues Found triad is load-bearing for st-flow's retry loop (Rule 4), which decides based on Status. Adding Criterion results alongside preserves both semantics.

### 4.10 Version bump

`python scripts/bump-version.py patch` lifts v0.19.1 → v0.19.2 across four targets in lock-step:

- `apps/stele/Cargo.toml` (workspace version — `Cargo.lock` refresh pulled by `cargo update --workspace` automatically).
- `plugins/stele/.claude-plugin/plugin.json`.
- `plugins/steop/.claude-plugin/plugin.json`.
- `apps/steop/version.go`.

Patch tier is correct per `docs/versioning.md` ("fixes and docs") — every change is additive skill-prose; no code, no schema, no API surface is modified.

## 5. Changes by Component

| Component                              | Change                                                                                                                                                                                              | Files                                                                                                                                     |
| -------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------- |
| st-flow Clarify override               | Replace the FLOW MODE blockquote per §4.1. Drop "Prefer making reasonable assumptions"; add explicit-state + multi-framing clauses.                                                                 | `plugins/steop/skills/st-flow/SKILL.md` (blockquote at line 59 pre-edit)                                                                  |
| st-flow Task Brief shape               | Insert explicit code block per §4.2 immediately after Clarify override.                                                                                                                             | `plugins/steop/skills/st-flow/SKILL.md` (new content after line 59)                                                                       |
| st-flow Plan override                  | Replace the FLOW MODE blockquote per §4.3. Soften approval-pause framing; add inline Alternatives-considered + YAGNI guidance.                                                                      | `plugins/steop/skills/st-flow/SKILL.md` (blockquote at line 99 pre-edit)                                                                  |
| st-flow Research override (new)        | Add a `> **FLOW MODE:**` blockquote per §4.4 requiring Assumptions at the end of Research Summary.                                                                                                   | `plugins/steop/skills/st-flow/SKILL.md` (new content after line 82 pre-edit)                                                              |
| st-flow Execute override (new)         | Add a `> **FLOW MODE:**` blockquote per §4.4 mirroring st-lite's YAGNI + surgical clauses.                                                                                                          | `plugins/steop/skills/st-flow/SKILL.md` (new content after line 116 pre-edit)                                                             |
| st-flow Validate override (new)        | Add a `> **FLOW MODE:**` blockquote per §4.4 anchoring against Success criteria.                                                                                                                    | `plugins/steop/skills/st-flow/SKILL.md` (new content after line 128 pre-edit)                                                             |
| st-clarify Brief + rule                | Insert `Success criteria` bullet in Brief shape; append multi-framing rule to Step 2 Clarify guidance.                                                                                              | `plugins/steop/skills/st-clarify/SKILL.md` (bullet list at lines 40–45 pre-edit; rules near line 35)                                       |
| st-research Summary                    | Add `Assumptions` bullet to Summary shape.                                                                                                                                                          | `plugins/steop/skills/st-research/SKILL.md` (bullet list at lines 37–42 pre-edit)                                                          |
| st-plan guidance + bullets             | Add YAGNI guidance paragraph; reshape Architecture decisions + Testing strategy bullets per §4.7.                                                                                                    | `plugins/steop/skills/st-plan/SKILL.md` (body around line 20; bullets at lines 22–29 pre-edit)                                             |
| st-execute Goals                       | Replace Execution Goals bullet list with prose paragraph per §4.8.                                                                                                                                   | `plugins/steop/skills/st-execute/SKILL.md` (bullet list at lines 32–36 pre-edit)                                                           |
| st-validate instructions + report      | Insert step 0 (Check Success criteria first); add `Criterion results` row to report shape.                                                                                                          | `plugins/steop/skills/st-validate/SKILL.md` (numbered list at lines 18–24 pre-edit; report at lines 26–31 pre-edit)                        |
| Workspace docs index (insert row)      | Add PRD-026 row to the PRD table, ordered numerically after PRD-025. Do NOT touch adjacent rows, even to fix known drift (per PRD-025 Author rule).                                                   | `docs/README.md` PRD table                                                                                                                |
| Version bump                           | Run `python scripts/bump-version.py patch`; verify all four targets moved to `0.19.2`.                                                                                                              | `apps/stele/Cargo.toml`, `plugins/stele/.claude-plugin/plugin.json`, `plugins/steop/.claude-plugin/plugin.json`, `apps/steop/version.go`  |

No source-code changes. No edits to `plugins/steop/agents/**`, `plugins/steop/hooks/**`, `plugins/steop/skills/st-prd/**`, `plugins/steop/skills/st-lite/**`, `plugins/steop/skills/st-send/**`, `plugins/steop/skills/st-watch/**`, `plugins/steop/skills/install/**`, `plugins/steop/skills/statusline-setup/**`, or any `.claude-plugin/` marketplace manifest outside the two plugin.json version fields.

## 6. Edge Cases

| Scenario                                                                                              | Behavior                                                                                                                                                                                                                                                                                                                                                                                    |
| ----------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| st-flow Clarify consultant returns a brief missing `Success criteria:`                                 | **Execute proceeds; Validate falls back to generics.** The Validate override (§4.4) reads "Check each bullet … A criterion is satisfied when you can observe it" — if the list is empty, zero bullets to check, and the generic build/run/regression check applies. No error state.                                                                                                          |
| st-flow Plan phase produces a blueprint with no Alternatives-considered lines                         | **No enforcement.** The override is guidance, not a hard schema. If the architect emits a plan with no alternatives noted, the executor still runs it. The `step → verify` retrospective is unaffected. Flag-and-soft-enforce.                                                                                                                                                                |
| Standalone `/steop:st-validate` invoked outside a flow (no Task Brief to reference)                    | **Step 0 is a no-op if no brief is present.** The st-validate step 0 says "Read the Task Brief's Success criteria (if available)" — if no brief exists in the conversation context, the reviewer agent proceeds to steps 1–5 as before. No behavior regression for standalone usage.                                                                                                         |
| st-flow Research override requires Assumptions but the researcher finds none to state                 | **"(none)" is acceptable.** Mirrors the st-lite zero-bullet convention from PRD-025 §4.1. The field is required as a line; its content can be `(none)`.                                                                                                                                                                                                                                     |
| Existing in-flight st-flow session started on v0.19.1, continues post-upgrade                         | **No session migration needed.** Skills are re-read on invocation; the next `/steop:st-flow` call picks up the new prose. Active sessions' Task Briefs are in-conversation artifacts, not persisted, so no backfill issue.                                                                                                                                                                   |
| A user invokes `/steop:st-execute` standalone with a plan that says "refactor all adjacent helpers"   | **The plan wins; the skill's surgical clause is ignored in that specific instruction.** The st-execute override says "Do NOT refactor neighboring code" but also says "Follow the plan step by step." If the plan explicitly demands neighboring refactors, the executor follows the plan — Karpathy P3 is guidance, not a veto over stated requirements.                                     |
| st-flow Execute-Validate retry loop hits the 3-round cap with criterion-anchored failures             | **Same stop behavior as today.** Rule 4 in st-flow still halts after 3 failed rounds. The Validate override adds "name the first unsatisfied criterion" so the halt report now carries a specific target ID — useful for the manual retry the user will do.                                                                                                                                  |
| User's existing wiki or onboarding doc references the old st-flow Clarify override text               | **External drift, not in scope.** Out-of-tree docs (team wikis, Notion pages, personal `~/.claude/CLAUDE.md` templates) may still quote the old "Prefer making reasonable assumptions" line. Only in-repo prose is swept. Document owners update on their own cadence.                                                                                                                        |
| A future skill is added that invokes the reviewer agent and expects no override                       | **Unaffected.** All overrides in this PRD are skill-local; the reviewer agent's default persona stays Karpathy-unaware. A new skill gets whatever it writes into its own override block. The PRD-025 Open Q #2 deferral still holds.                                                                                                                                                          |

## 7. Migration

**Non-breaking.** Every change is additive at the user-facing surface:

- Existing `/steop:st-flow` invocations still work. The Clarify override now produces a richer Task Brief; Execute and Validate phases now see explicit FLOW MODE blocks where previously they saw none. No downstream code or hook reads the Brief as structured data; the richer shape is consumed only by the next phase in the same conversation.
- Existing standalone skill invocations still work. New fields are additive; old callers don't break.
- The `--mode flow`, `--mode clarify`, `--mode research`, `--mode plan`, `--mode execute`, `--mode validate` CLI tags keep their opaque-string semantics. No `steop` binary or Go-side change.
- Agent files untouched — other skills in the plugin (`st-send`, `st-watch`, etc.) that happen to invoke these agents see no change.

**Version bump:**

```bash
cd /path/to/stele
python scripts/bump-version.py patch
# Verify:
grep -E '"?version"?\s*[:=]\s*"0\.19\.2"' \
  apps/stele/Cargo.toml \
  plugins/stele/.claude-plugin/plugin.json \
  plugins/steop/.claude-plugin/plugin.json
grep '"0.19.2"' apps/steop/version.go
```

Flip the top-of-file `**Status:**` on this PRD from `Proposed` to `Implemented (v0.19.2)` once §8 acceptance holds, and update the `docs/README.md` row for PRD-026 in the same commit.

## 8. Testing

No automated test harness is added — the skills are prose. Manual acceptance sequence in `step → verify:` format per PRD-025's Author override (dogfooded here):

### 8.1 File-system and version

- Run `ls plugins/steop/skills/{st-flow,st-clarify,st-research,st-plan,st-execute,st-validate}/SKILL.md` → **verify:** six paths exist.
- Run `python scripts/bump-version.py --list` → **verify:** workspace, stele, steop all read `0.19.2`.

### 8.2 st-flow conflict removal

- Run `grep -c 'Prefer making reasonable assumptions' plugins/steop/skills/st-flow/SKILL.md` → **verify:** `0`.
- Run `grep -c 'Do NOT present it for approval or ask for adjustments' plugins/steop/skills/st-flow/SKILL.md` → **verify:** `0`.
- Run `grep -c 'State assumptions explicitly' plugins/steop/skills/st-flow/SKILL.md` → **verify:** at least `1`.

### 8.3 st-flow Task Brief shape

- Run `grep -E '^(Objective|Assumptions|Complexity|Success criteria|Open questions|Groups):' plugins/steop/skills/st-flow/SKILL.md` → **verify:** six matches, one per field, in declared order.

### 8.4 st-flow new override blocks

- Run `grep -c '> \*\*FLOW MODE:\*\*' plugins/steop/skills/st-flow/SKILL.md` → **verify:** `5` (Clarify + Plan already exist; Research + Execute + Validate added by this PRD).
- Run `grep -c 'Prefer YAGNI' plugins/steop/skills/st-flow/SKILL.md` → **verify:** at least `1` (inside the new Execute override).
- Run `grep -c 'Success criteria' plugins/steop/skills/st-flow/SKILL.md` → **verify:** at least `2` (Task Brief shape + Validate override).

### 8.5 st-clarify shape sweep

- Run `grep -c '\*\*Success criteria\*\*' plugins/steop/skills/st-clarify/SKILL.md` → **verify:** at least `1`.
- Run `grep -c 'Present alternatives when the request has two or more plausible framings' plugins/steop/skills/st-clarify/SKILL.md` → **verify:** `1`.

### 8.6 st-research Summary sweep

- Run `grep -c '\*\*Assumptions\*\*' plugins/steop/skills/st-research/SKILL.md` → **verify:** at least `1`.

### 8.7 st-plan guidance sweep

- Run `grep -c 'simpler of two equivalent designs' plugins/steop/skills/st-plan/SKILL.md` → **verify:** `1`.
- Run `grep -c 'Alternative considered' plugins/steop/skills/st-plan/SKILL.md` → **verify:** at least `1`.
- Run `grep -c 'step → verify' plugins/steop/skills/st-plan/SKILL.md` → **verify:** at least `1`.

### 8.8 st-execute sweep

- Run `grep -c 'Prefer YAGNI' plugins/steop/skills/st-execute/SKILL.md` → **verify:** `1`.
- Run `grep -c 'Do NOT refactor neighboring code' plugins/steop/skills/st-execute/SKILL.md` → **verify:** `1`.
- Run `grep -c 'Only remove imports, variables, or functions that your changes made unused' plugins/steop/skills/st-execute/SKILL.md` → **verify:** `1`.

### 8.9 st-validate sweep

- Run `grep -c 'Check Success criteria first' plugins/steop/skills/st-validate/SKILL.md` → **verify:** `1`.
- Run `grep -c 'Criterion results' plugins/steop/skills/st-validate/SKILL.md` → **verify:** at least `1`.

### 8.10 Rust and Go build (unchanged)

- Run `cd apps/stele && cargo check -p stele-server -p stele-cli` → **verify:** clean build.
- Run `cd apps/steop && go build -o target/steop .` → **verify:** clean build.

### 8.11 Live st-flow dogfood

- In Claude Code, run `/steop:st-flow add a trailing newline to CHANGELOG.md` (or any one-file trivial task) → **verify:** Clarify emits a Task Brief including `Assumptions:` and `Success criteria:`. Validate's pass/fail reason references a criterion bullet by name.

### 8.12 Style match against PRD-025

- Run `diff <(grep '^##' docs/prd/prd-025-brief-fields-karpathy.md) <(grep '^##' docs/prd/prd-026-karpathy-pipeline-skills.md)` → **verify:** same top-level section names (modulo PRD-026's extra subsections — `##` headings should be identical).

## 9. Open Questions

1. **Should the `Open questions` field in st-flow's Task Brief eventually become a structured list tied to a resolver step?** Right now it surfaces at Finalize as informational prose. A future PRD could tie each `Open questions` entry to a follow-up `/steop:st-send` to the user for async resolution. Out of scope for v0.19.2; the field is useful as-is for human readers.
2. **Should `st-execute`'s plan-wins-over-skill rule (Edge Cases row 5) be codified or left as convention?** The executor currently follows the plan verbatim. If a plan demands neighboring refactors, st-execute's "Do NOT refactor neighboring code" clause is overridden by the plan's instruction. This is correct behavior (plans are load-bearing; skill guidance is default) but is only implicit in the prose. A future patch could add an explicit rule. Not blocking for v0.19.2.
3. **Should agent-file edits eventually land?** PRD-025 Open Q #2 deferred this and PRD-026 maintains the deferral. The case for centralizing keeps growing (every new skill has to re-declare the same Karpathy framing in its overrides). A follow-up PRD could evaluate whether to bake Karpathy defaults into `plugins/steop/agents/{consultant, executor, reviewer, architect, researcher}.md` — with a careful migration for any existing skill that expects the old defaults.
