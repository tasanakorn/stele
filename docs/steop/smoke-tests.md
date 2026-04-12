# Steop RPC smoke tests

Copy-paste curl sequence that exercises every `POST /api/v1/steop/*` method end-to-end. Use this to verify a running `stele-server` after a schema or RPC change.

For the contract itself (method signatures, identity model, schema), see [DESIGN.md](DESIGN.md).

## Setup

```bash
KEY=...
URL=http://127.0.0.1:3100/api/v1/steop
H="X-Stele-Key: $KEY"
CT="Content-Type: application/json"

# Composite identifiers. ID is a 3-segment session id; PID is the corresponding
# 2-segment project id derived from the same host:project_dir prefix.
UUID=a1b2c3d4-5678-4abc-9def-0123456789ab
ID="laptop:/tmp/demo:$UUID"
PID="laptop:/tmp/demo"
```

## Session lifecycle

```bash
curl -sS -X POST "$URL/steop.session.start" -H "$H" -H "$CT" \
  -d "{\"id\":\"$ID\",\"data\":{\"phase\":\"plan\"}}"

curl -sS -X POST "$URL/steop.session.touch" -H "$H" -H "$CT" \
  -d "{\"id\":\"$ID\"}"

curl -sS -X POST "$URL/steop.session.get" -H "$H" -H "$CT" \
  -d "{\"id\":\"$ID\"}"

curl -sS -X POST "$URL/steop.session.list" -H "$H" -H "$CT" \
  -d '{"host":"laptop","project_dir":"/tmp/demo","state":"active"}'

curl -sS -X POST "$URL/steop.project.list" -H "$H" -H "$CT" \
  -d '{"host":"laptop"}'
```

## State + counters

```bash
curl -sS -X POST "$URL/steop.state.put" -H "$H" -H "$CT" \
  -d "{\"id\":\"$ID\",\"data\":{\"phase\":\"execute\"},\"merge\":true}"

curl -sS -X POST "$URL/steop.state.incr" -H "$H" -H "$CT" \
  -d "{\"id\":\"$ID\",\"counter\":\"tool_calls\",\"delta\":1}"

curl -sS -X POST "$URL/steop.state.reset" -H "$H" -H "$CT" \
  -d "{\"id\":\"$ID\",\"counter\":\"tool_calls\",\"value\":0}"
```

## Status

```bash
curl -sS -X POST "$URL/steop.status.get" -H "$H" -H "$CT" \
  -d "{\"id\":\"$ID\"}"
```

## Storage (session-level, 3-segment id)

```bash
curl -sS -X POST "$URL/steop.storage.put" -H "$H" -H "$CT" \
  -d "{\"id\":\"$ID\",\"key\":\"plan\",\"content\":\"{\\\"steps\\\":[1,2,3]}\"}"

curl -sS -X POST "$URL/steop.storage.get" -H "$H" -H "$CT" \
  -d "{\"id\":\"$ID\",\"key\":\"plan\"}"

curl -sS -X POST "$URL/steop.storage.list" -H "$H" -H "$CT" \
  -d "{\"id\":\"$ID\"}"
```

## Storage (project-level, 2-segment id)

```bash
curl -sS -X POST "$URL/steop.storage.put" -H "$H" -H "$CT" \
  -d "{\"id\":\"$PID\",\"key\":\"brief\",\"content\":\"shared\"}"
```

## Log

```bash
curl -sS -X POST "$URL/steop.log.append" -H "$H" -H "$CT" \
  -d "{\"id\":\"$ID\",\"event\":\"post_tool_use\",\"data\":{\"tool_name\":\"Bash\"}}"

curl -sS -X POST "$URL/steop.log.query" -H "$H" -H "$CT" \
  -d '{"host":"laptop","project_dir":"/tmp/demo","limit":20}'
```

## Mailbox

```bash
# 1. Send with implicit from — server derives from from id.
curl -sS -X POST "$URL/steop.mailbox.send" -H "$H" -H "$CT" \
  -d "{\"id\":\"$ID\",\"to\":\"$PID\",\"subject\":\"demo message\",\"message_type\":\"NOTE:INFO\",\"payload\":{\"phase\":\"validate\",\"tool_calls\":42}}"
# Expect: {"message_id":1,"from":"...","to":"laptop:/tmp/demo","status":"NEW",...}

# 2. List inbox (implicit to — defaults to caller's id; default status filter = ["NEW"]).
curl -sS -X POST "$URL/steop.mailbox.list" -H "$H" -H "$CT" \
  -d "{\"id\":\"$PID\"}"
# Expect: {"messages":[{"message_id":1,...,"status":"NEW"}]}

# 3. Get single row — side-effect free (status stays NEW).
curl -sS -X POST "$URL/steop.mailbox.get" -H "$H" -H "$CT" \
  -d "{\"id\":\"$ID\",\"message_id\":1}"
# Expect: full MailboxRow with status:"NEW"

# 4. Read — NEW → READ.
curl -sS -X POST "$URL/steop.mailbox.read" -H "$H" -H "$CT" \
  -d "{\"id\":\"$ID\",\"message_id\":1}"
# Expect: {"message_id":1,"status":"READ"}

# 5. Read again — already READ, expect 409.
curl -sS -X POST "$URL/steop.mailbox.read" -H "$H" -H "$CT" \
  -d "{\"id\":\"$ID\",\"message_id\":1}"
# Expect: HTTP 409 {"error":"invalid mailbox status transition: READ -> READ"}

# 6. Send a second message; archive directly from NEW.
curl -sS -X POST "$URL/steop.mailbox.send" -H "$H" -H "$CT" \
  -d "{\"id\":\"$ID\",\"to\":\"$PID\",\"subject\":\"second\",\"message_type\":\"NOTE:INFO\"}"
# Expect: {"message_id":2,...,"status":"NEW"}
curl -sS -X POST "$URL/steop.mailbox.archive" -H "$H" -H "$CT" \
  -d "{\"id\":\"$ID\",\"message_id\":2}"
# Expect: {"message_id":2,"status":"ARCHIVE"}

# 7. Archive an already-archived row — expect 409.
curl -sS -X POST "$URL/steop.mailbox.archive" -H "$H" -H "$CT" \
  -d "{\"id\":\"$ID\",\"message_id\":2}"
# Expect: HTTP 409 {"error":"invalid mailbox status transition: ARCHIVE -> ARCHIVE"}

# 8. List with default filter after all messages are READ or ARCHIVE — expect empty.
curl -sS -X POST "$URL/steop.mailbox.list" -H "$H" -H "$CT" \
  -d "{\"id\":\"$PID\"}"
# Expect: {"messages":[]}

# 9. List with explicit status filter including archived.
curl -sS -X POST "$URL/steop.mailbox.list" -H "$H" -H "$CT" \
  -d "{\"id\":\"$PID\",\"status\":[\"NEW\",\"READ\",\"ARCHIVE\"]}"
# Expect: both message rows returned

# 10. Send to USER principal.
curl -sS -X POST "$URL/steop.mailbox.send" -H "$H" -H "$CT" \
  -d "{\"id\":\"$ID\",\"to\":\"$PID:USER\",\"subject\":\"notify human\",\"message_type\":\"NOTE:WARN\"}"
# Expect: {"message_id":3,...,"to":"laptop:/tmp/demo:USER","status":"NEW"}

# 11. Parser rejection — 3rd segment is neither UUID nor USER.
curl -sS -X POST "$URL/steop.mailbox.send" -H "$H" -H "$CT" \
  -d "{\"id\":\"$PID:alice\",\"to\":\"$PID\",\"subject\":\"bad id\"}"
# Expect: HTTP 400 {"error":"id 3rd segment must be a session UUID or the literal 'USER'"}
```

## Session stop

```bash
curl -sS -X POST "$URL/steop.session.stop" -H "$H" -H "$CT" \
  -d "{\"id\":\"$ID\"}"
```
