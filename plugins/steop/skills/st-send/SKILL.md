---
name: st-send
description: Send a task request to another Claude Code session by project name. Resolves short project names to full composite IDs automatically.
---

# Send Task to Another Session

Send a `TASK:REQUEST` message to another active Claude Code session. The target can be a short project directory name (e.g., `frontend`) which gets resolved to the correct session automatically.

## Usage

```bash
steop send <target> <description> [--mode=normal|flow] [--subject=SUBJECT] [--meta=JSON]
```

## Parameters

| Parameter     | Required | Default  | Description                                                 |
| ------------- | -------- | -------- | ----------------------------------------------------------- |
| `target`      | Yes      |          | Project name suffix or full composite ID                    |
| `description` | Yes      |          | Task description (all remaining positional args joined)     |
| `--mode`      | No       | `normal` | Execution mode: `normal` (plain) or `flow` (full pipeline)  |
| `--subject`   | No       | auto     | Message subject (defaults to first 80 chars of description) |
| `--meta`      | No       |          | Additional JSON merged into the message meta                |

## Target Resolution

- If `target` contains `:`, it is treated as a full composite ID (no resolution).
- Otherwise, active sessions on this host are searched for a matching `project_dir`:
  - Exact match or suffix match after `/` (e.g., `frontend` matches `/Users/dev/frontend`).
  - Ambiguous matches (multiple different project dirs) produce an error listing candidates.

## Examples

Send a simple task:
```bash
steop send frontend "Add a loading spinner to the dashboard page"
```

Send with flow mode for complex tasks:
```bash
steop send backend "Refactor the auth middleware to support JWT refresh tokens" --mode=flow
```

Send with a full composite ID:
```bash
steop send "hostname:/Users/dev/frontend:USER" "Fix the CSS regression in header"
```
