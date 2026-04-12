---
name: st-watch
description: Monitor mailbox for incoming task requests and process them autonomously. Starts a polling loop that watches for TASK:REQUEST messages, claims them, and routes them based on mode (st-flow for flow tasks, direct execution for normal tasks).
---

# Watch for Task Requests

Monitor the session's mailbox for incoming `TASK:REQUEST` messages. When a task arrives, claim it, process it, and report the result back to the sender.

## Step 1 — Start Watcher + Monitor

Launch the watcher and begin monitoring in a single turn. Issue both tool calls in parallel:

**Background process** (`run_in_background: true`):

```bash
steop mailbox watch --type TASK:REQUEST --interval 10
```

**Monitor tool**: stream stdout lines from the background watcher process. Each line is a complete JSON object representing a new `TASK:REQUEST` message.

## Step 2 — On Receiving a Task

Process one task at a time. Do not claim a new task while one is in progress.

### 2a. Parse the JSON line

Extract: `message_id`, `from`, `meta.task_id`, `meta.description`, and `payload`.

### 2b. Claim the task

```bash
steop mailbox read <message_id>
```

If the response is HTTP 409 (already claimed by another watcher), skip this task and continue monitoring.

### 2c. Track the active task

```bash
steop storage put watcher:active_tasks '[{"task_id":"<task_id>","request_message_id":<message_id>,"from":"<from>"}]'
```

### 2d. Send CHECKIN

```bash
steop mailbox send \
  --to=<from> \
  --type=TASK:CHECKIN \
  --meta='{"task_id":"<task_id>","request_message_id":<message_id>}'
```

### 2e. Process the task

Determine the execution mode from `meta.mode` (default to `"normal"` if absent or unrecognized):

- **`flow`** — Execute the task using `/steop:st-flow` with `meta.description` as the user request. Include `payload` as additional context if present.
- **`normal`** — Execute `meta.description` as a plain conversation turn (no pipeline). Include `payload` as additional context if present. Use your own judgment and available tools to complete the request directly.

### 2f. Report result

**On success:**

```bash
steop mailbox send \
  --to=<from> \
  --type=TASK:DONE \
  --subject="Completed: <description>" \
  --meta='{"task_id":"<task_id>","request_message_id":<message_id>}' \
  --payload='<result_summary_json>'
```

**On failure:**

```bash
steop mailbox send \
  --to=<from> \
  --type=TASK:FAILED \
  --subject="Failed: <description>" \
  --meta='{"task_id":"<task_id>","request_message_id":<message_id>,"error":"<error_summary>"}' \
  --payload='<error_details_json>'
```

### 2g. Archive the original request

```bash
steop mailbox archive <message_id>
```

### 2h. Cleanup active tasks

```bash
steop storage put watcher:active_tasks '[]'
```

## Step 3 — Continue Monitoring

Return to the Monitor tool to wait for the next task. Repeat from Step 2 for each new message.
