---
name: st-validate
description: Validate phase of the workflow chain. Uses Sonnet to review and validate implementation changes. Use when the user wants to validate that code changes are correct and complete.
---

# Validate Phase

Review and validate the implementation changes.

## Instructions

```bash
steop state set-phase validate --mode validate
```

Launch the **reviewer** agent (`steop:reviewer`, Sonnet, read-only tools).

The verification agent should:

1. **Review changes** — Read all modified/created files and verify they match the intended plan
2. **Check correctness** — Look for bugs, typos, logic errors, missing edge cases
3. **Run tests** — Execute any available test suites or linting tools relevant to the changes
4. **Check consistency** — Ensure changes follow existing codebase patterns and conventions
5. **Check completeness** — Verify nothing was missed from the plan

After the agent completes, present a verification report:
- **Status** — Pass / Fail / Issues Found
- **Changes reviewed** — list of files checked
- **Issues** — any problems found (with severity)
- **Test results** — output from tests/linting if applicable
- **Recommendations** — suggested fixes if issues were found
