---
name: st-watch
description: Monitor mailbox for incoming task requests and process them autonomously. Starts a polling loop that watches for TASK:REQUEST messages, claims them, and executes them via st-flow.
---

# Watch for Task Requests

Monitor the session's mailbox for incoming `TASK:REQUEST` messages. When a task arrives, claim it, process it via `/steop:st-flow`, and report the result back to the sender.

## Step 1 — Resume State

Check for a previous watcher checkpoint so restarted watchers don't re-emit old messages:

```bash
steop storage --session=$SESSION_ID get watcher:last_message_id
```

If found, note the `content` value as `LAST_MESSAGE_ID`. If not found or error, proceed without `--since`.

## Step 2 — Start the Watcher Process

Run the watcher as a background process using `run_in_background: true`:

```bash
steop mailbox watch --type TASK:REQUEST --interval 10 [--since=LAST_MESSAGE_ID]
```

Include `--since=LAST_MESSAGE_ID` only if Step 1 returned a value.

## Step 3 — Monitor for Incoming Tasks

Use the **Monitor** tool to stream stdout lines from the background watcher process. Each line is a complete JSON object representing a new `TASK:REQUEST` message.

## Step 4 — On Receiving a Task

Process one task at a time. Do not claim a new task while one is in progress.

### 4a. Parse the JSON line

Extract: `message_id`, `from`, `meta.task_id`, `meta.description`, and `payload`.

### 4b. Claim the task

```bash
steop mailbox read <message_id>
```

If the response is HTTP 409 (already claimed by another watcher), skip this task and continue monitoring.

### 4c. Track the active task

Read the current active tasks list:

```bash
steop storage --session=$SESSION_ID get watcher:active_tasks
```

Append the new task entry and write back:

```bash
steop storage --session=$SESSION_ID put watcher:active_tasks '[{"task_id":"<task_id>","request_message_id":<message_id>,"from":"<from>"}]'
```

### 4d. Send CHECKIN

```bash
steop mailbox send \
  --to=<from> \
  --type=TASK:CHECKIN \
  --meta='{"task_id":"<task_id>","request_message_id":<message_id>}'
```

### 4e. Process the task

Execute the task using `/steop:st-flow` with `meta.description` as the user request. Include `payload` as additional context if present.

### 4f. Report result

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

### 4g. Archive the original request

```bash
steop mailbox archive <message_id>
```

### 4h. Update tracking

Remove the completed task from `watcher:active_tasks` and update the checkpoint:

```bash
steop storage --session=$SESSION_ID put watcher:last_message_id '<message_id>'
```

## Step 5 — Continue Monitoring

Return to the Monitor tool to wait for the next task. Repeat from Step 4 for each new message.
