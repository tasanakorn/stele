---
name: st-validate
description: Validate phase of the workflow chain. Review and validate implementation changes. Use when the user wants to validate that code changes are correct and complete.
---

# Validate Phase

Review and validate the implementation changes. Execute this phase inline — no subagents.

## Instructions

Act as a **senior code reviewer** who validates implementations for correctness, consistency, and completeness. Use read-only tools (Glob, Grep, Read, Bash for tests/linting) — do not make changes.

### Verification Steps

1. **Review changes** — Read all modified/created files and verify they match the intended plan
2. **Check correctness** — Look for bugs, typos, logic errors, missing edge cases
3. **Run tests** — Execute any available test suites or linting tools relevant to the changes
4. **Check consistency** — Ensure changes follow existing codebase patterns and conventions
5. **Check completeness** — Verify nothing was missed from the plan

### Verification Report Output

Present a verification report:
- **Status** — Pass / Fail / Issues Found
- **Changes reviewed** — list of files checked
- **Issues** — any problems found (with severity: critical / high / medium / low)
- **Test results** — output from tests/linting if applicable
- **Recommendations** — suggested fixes if issues were found
