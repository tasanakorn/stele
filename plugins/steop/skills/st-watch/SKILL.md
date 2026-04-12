---
name: st-watch
description: Monitor mailbox for incoming task requests and process them autonomously. Starts a polling loop that watches for TASK:REQUEST messages, claims them, and routes them based on mode.
---

# Watch for Task Requests

Monitor the session's mailbox for incoming `TASK:REQUEST` messages. When a task arrives, claim it, process it, and report the result back to the sender.

## Step 1 — Start Watcher + Monitor

Issue both tool calls in parallel in a single turn:

1. **Bash** (`run_in_background: true`): `steop mailbox watch --type TASK:REQUEST --interval 10`
2. **Monitor**: stream stdout from the background watcher process.

The first line will be `{"type":"ready",...}` confirming the watcher initialized.
Subsequent lines are NDJSON task messages — proceed to Step 2 for each.

## Step 2 — On Receiving a Task

Process one task at a time. Do not claim a new task while one is in progress.

### 2a. Parse the JSON line

Extract: `message_id`, `from`, `meta.task_id`, `meta.description`, and `payload`.

### 2b. Claim the task

```bash
steop mailbox read <message_id>
```

If HTTP 409 (already claimed), skip this task and continue monitoring.

### 2c. Track + notify

```bash
steop storage put watcher:active_tasks '[{"task_id":"<task_id>","request_message_id":<message_id>,"from":"<from>"}]'
steop mailbox send --to=<from> --type=TASK:CHECKIN --meta='{"task_id":"<task_id>","request_message_id":<message_id>}'
```

### 2d. Process the task

Determine execution mode from `meta.mode` (default `"normal"`):

- **`flow`** — Run `/steop:st-flow` with `meta.description`. Include `payload` as context.
- **`normal`** — Execute `meta.description` directly. Include `payload` as context.

### 2e. Report result + cleanup

Send result back to `<from>`, archive the request, and clear active tasks:

```bash
# On success:
steop mailbox send --to=<from> --type=TASK:DONE --subject="Completed: <desc>" \
  --meta='{"task_id":"<task_id>","request_message_id":<message_id>}' --payload='<result_json>'
# On failure:
steop mailbox send --to=<from> --type=TASK:FAILED --subject="Failed: <desc>" \
  --meta='{"task_id":"<task_id>","request_message_id":<message_id>,"error":"<err>"}' --payload='<error_json>'

steop mailbox archive <message_id>
steop storage put watcher:active_tasks '[]'
```

## Step 3 — Continue Monitoring

Return to the Monitor tool to wait for the next task. Repeat from Step 2 for each new message.
