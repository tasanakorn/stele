# PRD-024 — Rename `/steop:st-xp` to `/steop:st-lite`

- **Status:** Proposed
- **Target version:** v0.19.0
- **Scope:** `plugins/steop/skills/st-xp/` directory rename to `plugins/steop/skills/st-lite/`, rewrite of its `SKILL.md` body to drop all "XP" framing, the mode tag `--mode xp` → `--mode lite` (statusline `[xp]` → `[lite]`), cross-references in `CLAUDE.md`, `plugins/steop/README.md`, `docs/README.md`, and a supersession admonition on `docs/prd/prd-018-st-xp-skill.md`. Lock-step version bump v0.18.0 → v0.19.0 via `python scripts/bump-version.py minor`.
- **Author:** Tasanakorn (design) + Claude Code (PRD authoring)

---

## 1. Goals

1. **Rename the skill path end-to-end.** The directory `plugins/steop/skills/st-xp/` moves to `plugins/steop/skills/st-lite/`. The user-facing slash command changes from `/steop:st-xp` to `/steop:st-lite`. After this PRD ships, invoking `/steop:st-xp` in Claude Code fails with "skill not found".
2. **Drop the "XP" naming from the workflow vocabulary.** The skill description, body prose, heading, statusline tag, and every inline example stop referencing Agile XP. The rename is substantive, not just a path swap — the skill's identity in prose is `lite`, not `xp`.
3. **Reassign the mode tag** from `--mode xp` to `--mode lite`. Statusline renders `[lite]` during a `st-lite` pipeline run. This is a surface-level rename; `steop state set-phase --mode <value>` keeps its opaque-string semantics and needs no Go-side code change.
4. **Record the rename as a supersession of PRD-018**, not a retraction. PRD-018's body is preserved verbatim as the historical record of the skill's original design intent; only an admonition pointing at PRD-024 is added at the top of the file.
5. **Lock-step minor version bump** to `v0.19.0` via `python scripts/bump-version.py minor`, propagating to `apps/stele/Cargo.toml`, `plugins/stele/.claude-plugin/plugin.json`, `plugins/steop/.claude-plugin/plugin.json`, and `apps/steop/version.go`.

## 2. Non-goals

- **No change to the pipeline's runtime behavior.** Acceptance signals, the retry loop, build checks, parallelism semantics, the cap-3 executor limit, and every other behavioral contract from PRD-018 stay exactly as implemented in v0.14.0. This PRD is a rename only.
- **No change to the `--mode` flag itself** or to the `steop state set-phase` CLI grammar. The flag's name, position, and parsing stay as documented in PRD-018.
- **No backwards-compatibility alias.** `/steop:st-xp` stops working immediately on install of v0.19.0. A shim directory pointing at `st-lite` is explicitly rejected (see §4.1 alternatives considered).
- **No back-fill of `CHANGELOG.md`** for historical `st-xp` entries. Past `st-xp` work remains accurate history — rewriting it would destroy the audit trail.
- **No withdrawal or deprecation of PRD-018** as a historical document. PRD-018 stays verbatim with only a new admonition added.
- **No fix** for the pre-existing stale line-reference inside PRD-018 body (`cmd_state.go:105-130` drift — now at `:141-171`). Out of scope for this rename; flag-and-defer.
- **No Go or Rust source changes.** `apps/steop/cmd_state.go`, `cmd_statusline.go`, and `internal/store/state.go` all treat `mode` as an opaque passthrough string; confirmed by research with zero hardcoded `"xp"` occurrences in `apps/`.

## 3. Background & Motivation

### 3.1 Current state

[PRD-018](prd-018-st-xp-skill.md) (Implemented v0.14.0) introduced `/steop:st-xp` as a compressed 3-phase Claude Code skill — `Clarify → Execute → Validate` — sitting alongside the full 5-phase `/steop:st-flow`. The naming choice riffed on Extreme Programming's fast-feedback ethos: smallest-slice-first, opt-in parallel exploration, cap-3 concurrent executors, no built-in retry.

Six months on, two independent frictions have accumulated:

- **Nomenclature collision with Agile XP.** New contributors reading the skill catalog ask whether `/steop:st-xp` is scoped to Agile-XP-shop users or implies pair programming, TDD-first, or other canonical XP practices. The skill has none of those couplings — the only borrowed idea is "fast feedback". Every time the question is asked, the answer is a micro-correction that disrupts onboarding. The name is misleading by accident.
- **No second XP-themed skill to anchor the prefix.** If `st-xp` were one of a family (`st-xp-clarify`, `st-xp-build`, etc.), the prefix would carry weight. It is not — it is a standalone compressed-pipeline skill. The `xp` token earns its complexity budget twice: once as the skill name, once as the `[xp]` mode tag in statusline. Both uses are single-point references, both are easy to rename.

The `lite` framing is neutral and self-describing: this is the pipeline you pick when you want something lighter than `st-flow`. No methodology baggage, no prefix that implies a family that does not exist, no collision with published software-engineering terminology.

### 3.2 Why now

- **PRD-018 is still "Proposed" in `docs/README.md`.** The row for PRD-018 at `docs/README.md:42` still says `Proposed`, though the PRD body and `apps/steop/version.go` both moved to `v0.14.0` at implementation time. Editing that table row to insert the new PRD-024 line is a natural moment to correct the drift (§7 Migration).
- **Zero code debt.** Research confirms there is no `"xp"` string literal in `apps/steop` Go code, nor in `apps/stele` Rust code, nor in any agent, hook, or marketplace manifest. The full blast radius is five Markdown files plus one directory rename plus one version-bump invocation. Deferring the rename only widens the doc-to-reality gap.
- **No in-flight PRDs or branches depend on the `st-xp` name.** PRD-019 through PRD-023 are all stylos/steop-backend scoped and do not reference the XP skill. Renaming now avoids cascading edits in future PRDs that would otherwise mention the old name.

## 4. Design

### 4.1 Hard rename, no alias

The skill directory `plugins/steop/skills/st-xp/` is renamed to `plugins/steop/skills/st-lite/`. The old path disappears from the repository. There is no shim directory, no alias file, and no redirect:

- **Alternative considered: compat-shim skill at `st-xp/SKILL.md` that delegates to `st-lite`.** Rejected. Claude Code skills are discovered by directory scan; a shim would mean two directories on disk, two rows in `plugins/steop/README.md`, and two entries in the marketplace view. The point of the rename is to reduce naming surface, not double it. A hard break for the ~6-month-old skill name is cheap.
- **Alternative considered: keep `st-xp` as a symlink to `st-lite`.** Rejected. Symlinks in `.claude-plugin` skill directories have no documented behavior in Claude Code; the plugin marketplace loader may treat them as independent skills, as duplicates, or as missing. Uncharted territory; not worth the experiment for a cosmetic compat.

Post-rename, a user who types `/steop:st-xp` sees the Claude Code slash-command autocomplete fail to match, same as any other undefined skill.

### 4.2 SKILL.md body rewrite

Every occurrence of `xp`, `XP`, `Xp` in `plugins/steop/skills/st-xp/SKILL.md` body is rewritten to `lite` / `Lite` / `LITE` with the respective casing. The following lines (numbering against the file pre-rename) are the complete edit set:

| Line  | Before (abbrev.)                                                                   | After (abbrev.)                                                   |
| ----- | ---------------------------------------------------------------------------------- | ------------------------------------------------------------------ |
| 2     | `name: st-xp`                                                                      | `name: st-lite`                                                    |
| 3     | `description: XP-style fast-feedback workflow chain. ...`                         | `description: Lite fast-feedback workflow chain. ...` (drop "XP")  |
| 6     | `# XP Workflow Chain`                                                              | `# Lite Workflow Chain`                                            |
| 8     | `--mode xp` (+ "XP signals compressed semantics")                                  | `--mode lite` (drop XP framing sentence)                           |
| 10    | `Use st-xp for:`                                                                   | `Use st-lite for:`                                                 |
| 14    | `[xp] Clarify: <objective>`                                                        | `[lite] Clarify: <objective>`                                      |
| 23    | `rerun st-xp`                                                                      | `rerun st-lite`                                                    |
| 25    | `running inside st-xp`                                                             | `running inside st-lite`                                           |
| 29    | `--mode xp`, `[xp] <phase>: <detail>`                                              | `--mode lite`, `[lite] <phase>: <detail>`                          |
| 48    | `set-phase clarify --mode xp`                                                      | `set-phase clarify --mode lite`                                    |
| 53    | `> **XP MODE:**`                                                                   | `> **LITE MODE:**`                                                 |
| 64    | `[xp] Clarify: ...`                                                                | `[lite] Clarify: ...`                                              |
| 71    | `set-phase execute --mode xp`                                                      | `set-phase execute --mode lite`                                    |
| 76    | `> **XP MODE:**`                                                                   | `> **LITE MODE:**`                                                 |
| 84    | `[xp] Execute: <N> parallel`, `[xp] Execute: 1`                                    | `[lite] Execute: <N> parallel`, `[lite] Execute: 1`                |
| 91    | `set-phase validate --mode xp`                                                     | `set-phase validate --mode lite`                                   |
| 96    | `> **XP MODE:**`                                                                   | `> **LITE MODE:**`                                                 |
| 98–99 | `[xp] Validate: pass`, `[xp] Validate: fail`                                       | `[lite] Validate: pass`, `[lite] Validate: fail`                   |
| 108   | `full st-flow would have checked but XP skipped`                                   | `full st-flow would have checked but st-lite skipped` (drop "XP")  |
| 110   | `XP is exploratory`                                                                | `st-lite is exploratory` (or a neutral rephrase; drop "XP")        |

The semantic contract (Clarify-Execute-Validate, cap-3 parallelism, no retry, optional `--explore`, etc.) is preserved unchanged. Only the identifier "XP"/"xp" is swept.

### 4.3 Mode tag flip

The statusline renders the session's persisted `mode` value verbatim inside square brackets. After this PRD:

- `steop state set-phase <phase> --mode lite` persists `mode="lite"` in the session record.
- Statusline shows `[lite]` during any `st-lite` phase.
- The `--mode` flag name, position, and parsing stay untouched. `apps/steop/cmd_state.go:141-171`, `apps/steop/cmd_statusline.go:215`, and `apps/steop/internal/store/state.go:205-207` all treat the value as an opaque string and require **no** code change.

### 4.4 Supersession admonition on PRD-018

A blockquote admonition is inserted directly under the `**Status:**` line (at `docs/prd/prd-018-st-xp-skill.md:3`, immediately after the existing status line). Pattern matches the style established at `docs/prd/prd-022-stylos-in-stele-server.md:131` and `docs/prd/prd-019-stylos-foundation.md:86`:

```
> **Superseded by [PRD-024](prd-024-rename-st-xp-to-st-lite.md) (v0.19.0).** The skill is renamed from `/steop:st-xp` to `/steop:st-lite`; mode tag flips from `--mode xp` to `--mode lite`. Pipeline behavior is unchanged.
```

PRD-018's body is **not** rewritten. The design rationale, the YAGNI-first framing, the `[xp]` examples in its body, and the ~6-month-old decision record all stay verbatim. The admonition is the only edit.

### 4.5 Cross-reference edits

Three other files carry explicit `st-xp` references that must track the rename:

- **`CLAUDE.md:235`** — the skills bullet `- **"/steop:st-xp"** — XP pipeline: clarify -> execute -> validate ...` is rewritten to `- **"/steop:st-lite"** — Lite pipeline: clarify -> execute -> validate ...` (or equivalent neutral description).
- **`CLAUDE.md:247`** — the plugin-structure code block `...,st-watch,st-send,st-prd,st-xp}/SKILL.md` becomes `...,st-watch,st-send,st-prd,st-lite}/SKILL.md`.
- **`plugins/steop/README.md:41`** — the table row `| XP | /steop:st-xp | Compressed 3-phase pipeline ... |` has its column label, command column, and description rewritten: `| Lite | /steop:st-lite | Compressed 3-phase pipeline ... |` (or equivalent — drop "XP" from the description text too).

### 4.6 Version bump

`python scripts/bump-version.py minor` lifts v0.18.0 → v0.19.0 across four targets in lock-step:

- `apps/stele/Cargo.toml` (workspace version — pulls `apps/stele/Cargo.lock` forward automatically on next cargo build).
- `plugins/stele/.claude-plugin/plugin.json`.
- `plugins/steop/.claude-plugin/plugin.json`.
- `apps/steop/version.go`.

A rename-only change is a **minor** bump under the workspace SemVer convention at `docs/versioning.md`: it is a user-visible contract shift (the skill name) without an API or DB break. Matches the precedent at PRD-023 (UDP default, also minor).

## 5. Changes by Component

| Component                          | Change                                                                                                                                           | Files                                                                                                   |
| ---------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------ | -------------------------------------------------------------------------------------------------------- |
| Skill directory rename             | `git mv` the directory. Path changes end-to-end; no file content preserved outside `SKILL.md`.                                                  | `plugins/steop/skills/st-xp/` → `plugins/steop/skills/st-lite/`                                          |
| Skill body rewrite                 | Strip every `xp`/`XP` token per §4.2 table; rewrite description line, heading, prose, statusline tags, mode flag examples, admonition headings. | `plugins/steop/skills/st-lite/SKILL.md` (post-rename path)                                               |
| Root `CLAUDE.md` skills list       | Rewrite the `/steop:st-xp` bullet description and the plugin-structure code block's skill enumeration.                                          | `CLAUDE.md` (lines ≈235 and ≈247 pre-edit)                                                               |
| Plugin README table                | Rewrite the `XP` row: column label, `/steop:st-xp` command cell, and description prose.                                                         | `plugins/steop/README.md` (line ≈41 pre-edit)                                                            |
| Workspace docs index (insert row)  | Add PRD-024 row to the PRD table, ordered numerically after PRD-023.                                                                            | `docs/README.md` PRD table (table at lines ≈24–47 pre-edit)                                              |
| Workspace docs index (drift fix)   | Update PRD-018 row's `Status` column from `Proposed` to `Implemented v0.14.0`, aligning with `docs/prd/prd-018-st-xp-skill.md:3`. Also reword the description cell to reflect the rename (e.g. "XP-style fast-feedback workflow skill — renamed to /steop:st-lite in v0.19.0 (see PRD-024)"). | `docs/README.md` PRD table, PRD-018 row (line ≈42 pre-edit)                                               |
| PRD-018 supersession admonition    | Insert blockquote admonition under `**Status:**` line per §4.4. Do not edit body prose.                                                         | `docs/prd/prd-018-st-xp-skill.md` (new content at line ≈4, pushing subsequent lines down)                |
| Version bump                       | Run `python scripts/bump-version.py minor`; verify all four targets moved to `0.19.0`.                                                          | `apps/stele/Cargo.toml`, `plugins/stele/.claude-plugin/plugin.json`, `plugins/steop/.claude-plugin/plugin.json`, `apps/steop/version.go` |

No source code changes. No changes to: `apps/steop/cmd_*.go`, `apps/steop/internal/**`, any `apps/stele/crates/**` source, any agent prompt in `plugins/steop/agents/`, any hook file in `plugins/steop/hooks/`, any marketplace manifest at `.claude-plugin/`.

## 6. Edge Cases

| Scenario                                                                          | Behavior                                                                                                                                                                                                                                                                                      |
| --------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Active steop session has `mode="xp"` persisted in local SQLite at upgrade time    | **Cosmetic transient.** The local-SQLite layer at `~/.local/share/steop/steop.db` stores `mode` as a value inside the per-session `data` JSON blob (not a typed column). The session keeps rendering `[xp]` in statusline until its state is overwritten by the next `steop state set-phase --mode lite` call or cleared via `steop state clear-phase`. No correctness impact. Documented in §7 Migration as a user-remediation step. |
| User types `/steop:st-xp` in Claude Code post-upgrade                              | **Hard miss.** Claude Code's slash-command autocomplete fails to match. User sees "no matching skill" UI, same as any undefined skill name. This is the load-bearing breaking change that justifies the minor bump.                                                                            |
| User has a muscle-memory note / wiki / onboarding doc mentioning `/steop:st-xp`   | **External drift, not in scope.** Out-of-tree docs (team wikis, Notion pages, `~/.claude/CLAUDE.md`) may still reference the old name until updated by their owners. Only in-repo references are swept by this PRD.                                                                            |
| User had a custom statusline regex matching `\[xp\]`                               | **Silent mismatch.** Any external tool that regex-matches the statusline tag `[xp]` stops firing. No in-tree statusline-consumer has such a regex; the `apps/steop/cmd_statusline.go` renderer is a producer, not a matcher. External consumers are out of scope — analogous to the `STELE_STYLOS_NO_QUIC` env var treatment in PRD-023. |
| A `/steop:st-xp` hook or automation in a parent repo                              | **Out of scope.** Hooks registered in `plugins/steop/hooks/hooks.json` target event names (PreToolUse, SessionStart, etc.), not skill names. Research confirmed zero hook references to `st-xp`. Any external repo that built a hook against `/steop:st-xp` as a trigger must adapt independently. |
| Invocation of `/steop:st-flow` is unaffected                                       | **No change.** The full 5-phase pipeline lives at `plugins/steop/skills/st-flow/SKILL.md` with `--mode flow` (or no mode tag). This PRD does not touch `st-flow` or any other skill directory.                                                                                                 |
| Plugin marketplace cache for the old skill name                                    | **User-side refresh.** Some Claude Code installs cache the plugin skill catalog. Users upgrading in-place may need to run the marketplace-refresh recipe documented at `docs/plugin-marketplace-troubleshooting.md` if `/steop:st-xp` autocompletes-but-404s after install of v0.19.0. Rare; flagged for §7.          |

## 7. Migration

**Breaking change in this PRD:**

1. **`/steop:st-xp` stops working.** After installing v0.19.0, the old slash command no longer resolves. Users must invoke `/steop:st-lite` instead. There is no alias and no deprecation warning.
2. **`--mode xp` no longer a convention.** Any automation or wrapper script that passes `--mode xp` to `steop state set-phase` still works syntactically (the CLI treats `--mode` as opaque) but produces a `[xp]` statusline tag divorced from any current skill. Updating scripts to `--mode lite` keeps the tag aligned with the skill name.

**Non-breaking:**

- `/steop:st-flow`, `/steop:st-clarify`, `/steop:st-research`, `/steop:st-plan`, `/steop:st-execute`, `/steop:st-validate`, `/steop:st-send`, `/steop:st-watch`, `/steop:st-prd`, `/steop:install` are all unchanged.
- The `steop state set-phase --mode <value>` CLI grammar is unchanged. `--mode` still accepts any string.
- PRD-018's body and its rationale stay available as a historical record.
- Server, MCP, REST, mailbox, stylos surfaces are untouched.

**Active-session remediation at upgrade time:** users with a live steop session whose persisted `mode` is `"xp"` will keep seeing `[xp]` in statusline until that session ends or is cleared. Two painless fixes:

```bash
# Easiest — clear the phase and let st-lite re-set it on next run.
steop state clear-phase

# Or overwrite directly.
steop state set-phase clarify --mode lite
```

**Historical-record preservation:** PRD-018 stays verbatim. Only a top-of-file supersession admonition is added, matching the style established by PRD-023 on PRDs 019 and 022. The admonition is informational; no PRD-018 body content is rewritten. This matches the workspace's decision-record-preservation norm from `docs/architecture.md` implicit convention and PRD-023 §4.8.

**Incidental drift fix:** `docs/README.md:42` still shows PRD-018 as `Proposed` despite its body reading `**Status:** Implemented (v0.14.0)` since that PRD shipped. Because this rename PRD must edit the same table row anyway (to reword the description), the status cell is corrected to `Implemented v0.14.0` in the same edit. Flag-and-defer was the alternative; fixing inline is cheaper and leaves the table self-consistent.

**Version bump:**

```bash
cd /path/to/stele
python scripts/bump-version.py minor
# Verify:
grep -E '"?version"?\s*[:=]\s*"0\.19\.0"' \
  apps/stele/Cargo.toml \
  plugins/stele/.claude-plugin/plugin.json \
  plugins/steop/.claude-plugin/plugin.json
grep '"0.19.0"' apps/steop/version.go
```

Flip the top-of-file `**Status:**` on this PRD from `Proposed` to `Implemented (v0.19.0)` once §8 acceptance holds, and update the `docs/README.md` row for PRD-024 in the same commit.

## 8. Testing

No automated test harness is added. Manual smoke-check sequence:

### 8.1 File-system rename verification

```bash
cd /path/to/stele
ls plugins/steop/skills/st-lite/SKILL.md      # Expected: file exists.
test ! -e plugins/steop/skills/st-xp         # Expected: true (directory gone).
head -3 plugins/steop/skills/st-lite/SKILL.md # Expected: frontmatter shows `name: st-lite`.
```

### 8.2 Reference-scrub verification

```bash
rg -n '\bst-xp\b|\[xp\]|--mode xp|XP MODE' \
  plugins/ CLAUDE.md docs/README.md plugins/steop/README.md
# Expected: zero matches. Matches inside docs/prd/prd-018-st-xp-skill.md
# are intentional historical content and are acceptable — that file is
# explicitly NOT included in the grep path list.
```

A single sweep scope matches is also an acceptance check:

```bash
rg -n 'st-xp|\[xp\]|--mode xp|XP' docs/prd/prd-018-st-xp-skill.md
# Expected: non-zero (historical); plus the new supersession admonition
# line linking to PRD-024.
```

### 8.3 Version-bump verification

```bash
python scripts/bump-version.py --list   # or equivalent inspection command
# Expected: all four tracked versions read 0.19.0.
```

### 8.4 Go build (unchanged)

```bash
cd apps/steop
go build -o target/steop .
# Expected: clean build. No Go source touched; this is a smoke check
# that the repo still compiles post-rename.
```

### 8.5 Rust build (unchanged)

```bash
cd apps/stele
cargo build -p stele-server
cargo build -p stele-cli
# Expected: clean builds. No Rust source touched.
```

### 8.6 Live skill invocation

In Claude Code, invoke the renamed skill on a trivial task:

```
/steop:st-lite Add a trailing newline to README.md
```

Expected: skill resolves; statusline shows `[lite] Clarify: …` → `[lite] Execute: …` → `[lite] Validate: pass` (or `fail`). No `[xp]` tag appears at any point.

### 8.7 Negative: old skill name

```
/steop:st-xp Add a trailing newline to README.md
```

Expected: Claude Code slash-command UI reports no matching skill. Exit with no side-effect.

### 8.8 Active-session migration

Start a session with the old mode persisted (simulated via direct `state set-phase`):

```bash
steop state set-phase clarify --mode xp
steop status   # Expected: shows [xp] — pre-rename state.

steop state clear-phase
# ... invoke /steop:st-lite and confirm statusline renders [lite].
```

### 8.9 PRD-018 admonition rendering

Preview `docs/prd/prd-018-st-xp-skill.md` in any Markdown viewer; confirm the supersession blockquote renders directly under the `**Status:**` line and links to `prd-024-rename-st-xp-to-st-lite.md` correctly.

## 9. Open Questions

1. **Should the skill description retain the "fast-feedback" phrase or re-anchor on "lite"?** §4.2 leans toward dropping Agile-XP-inflected language entirely (no "fast-feedback" slogan either). Alternative: keep "fast-feedback" since it accurately describes the pipeline. Decision deferred to the implementer's taste during the SKILL.md rewrite; both are consistent with this PRD's goals.
2. **Is a second `st-lite`-family skill likely?** If yes (`st-lite-send`, `st-lite-watch`), the `lite` prefix earns a family. If no, `st-lite` stays a standalone name. Not a blocker for v0.19.0; name is renamable again later at the same cost as this PRD (minor bump, ~5 files).
3. **Should the CHANGELOG mention the rename explicitly?** The workspace convention (per `docs/versioning.md`) relies on commit messages and PRD numbers for history. An explicit CHANGELOG line for v0.19.0 ("Rename /steop:st-xp to /steop:st-lite — see PRD-024") would help non-PRD-readers. Out of scope here; deferred to the implementing commit message.
