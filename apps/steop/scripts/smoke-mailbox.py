#!/usr/bin/env python3
"""Smoke test: `steop mailbox watch` receives a `steop send` TASK:REQUEST.

Not run in CI. Requires a live stele-server configured via the default profile.
"""

import json, os, socket, subprocess, time, uuid

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
    # Give the watcher a moment to initialize before sending.
    time.sleep(1)

    subprocess.run(["steop", "send", TO, SUBJECT], env=env, check=True,
                   capture_output=True)

    watcher.stdout.reconfigure(line_buffering=True)  # type: ignore[attr-defined]
    deadline = time.monotonic() + 10
    event = None
    while time.monotonic() < deadline:
        import select
        ready_fds, _, _ = select.select([stdout], [], [], deadline - time.monotonic())
        if not ready_fds:
            break
        line = stdout.readline()
        if not line:
            break
        try:
            parsed = json.loads(line)
        except json.JSONDecodeError:
            continue
        # Skip lifecycle lines (e.g. WATCHER:READY) — we only want the task.
        if parsed.get("message_type") != "TASK:REQUEST":
            continue
        event = parsed
        break

    assert event is not None, "timed out waiting for watcher event"
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
