#!/usr/bin/env python3
"""Smoke test: `steop mailbox watch` receives a `steop send` TASK:REQUEST.

Not run in CI. Requires a live stele-server configured via the default profile.
"""

import json, os, socket, subprocess, uuid

HOST = socket.gethostname().split(".")[0]
PROJECT = "/tmp/steop-smoke"
SID = str(uuid.uuid4())
TO = f"{HOST}:{PROJECT}:{SID}"
SUBJECT = f"smoke-{uuid.uuid4()}"

env = {**os.environ, "CLAUDE_PROJECT_DIR": PROJECT}

watcher = subprocess.Popen(
    ["steop", "mailbox", "watch", "--type=TASK:REQUEST", "--interval=5",
     f"--x-session-id={SID}", f"--x-project-dir={PROJECT}"],
    stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True, bufsize=1, env=env,
)
stdout = watcher.stdout
assert stdout is not None
msg_id = None
try:
    ready = json.loads(stdout.readline())
    assert ready.get("type") == "ready", f"expected ready, got {ready}"

    subprocess.run(["steop", "send", TO, SUBJECT], env=env, check=True,
                   capture_output=True)

    event = json.loads(stdout.readline())
    assert event["subject"] == SUBJECT, f"subject mismatch: {event}"
    assert event["message_type"] == "TASK:REQUEST", f"type mismatch: {event}"
    msg_id = event["message_id"]
    print(f"PASS: watcher received subject={SUBJECT!r} message_id={msg_id}")
finally:
    watcher.terminate()
    watcher.wait(timeout=5)
    if msg_id is not None:
        subprocess.run(["steop", "mailbox", "archive", str(msg_id)],
                       capture_output=True, env=env)
