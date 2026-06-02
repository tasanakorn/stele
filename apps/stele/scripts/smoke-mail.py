#!/usr/bin/env python3
"""Smoke test: PRD-027 postal mailbox via the `stele mail` CLI.

TEST-FIRST: this encodes the §8 acceptance steps for PRD-027. It will FAIL until
the feature is implemented (`stele mail` requires `--features mail`). Not run in
CI. Requires a live local stele-server with the `stylos` feature, reachable on
the default zenoh endpoint (tcp/127.0.0.1:31747).

Single-host steps run automatically. Cross-node delivery (steps 5-7 of the PRD)
needs a second machine on the same realm: set PEER_HOST=<other-host> to exercise
the outbound spool path (delivery on the peer is checked manually).

Usage:
    python apps/stele/scripts/smoke-mail.py
    PEER_HOST=vm-02 python apps/stele/scripts/smoke-mail.py
"""

import json, os, socket, subprocess, sys, uuid

HOST = socket.gethostname()
PROJECT = f"/tmp/stele-mail-smoke-{uuid.uuid4().hex[:8]}"
ALIAS = "claude"
env = {**os.environ, "CLAUDE_PROJECT_DIR": PROJECT}

created = []  # message_ids to archive on cleanup


def mail(*args, expect_code=0):
    """Run `stele mail <args>`, assert exit code, return parsed JSON stdout."""
    proc = subprocess.run(
        ["stele", "mail", *map(str, args)],
        capture_output=True, text=True, env=env,
    )
    if proc.returncode != expect_code:
        raise AssertionError(
            f"`stele mail {' '.join(map(str, args))}` exited {proc.returncode} "
            f"(expected {expect_code})\nstdout: {proc.stdout}\nstderr: {proc.stderr}"
        )
    return json.loads(proc.stdout) if proc.stdout.strip() else {}


def check(cond, label):
    if not cond:
        raise AssertionError(f"FAIL: {label}")
    print(f"  ok: {label}")


def main():
    global created
    print(f"host={HOST} project={PROJECT}")

    # 1. Local send (to self) → stored directly, status delivered.
    r = mail("send", "--to-host", HOST, "--to-project", PROJECT, "--subject", "hi")
    check(r.get("status") == "delivered", "local send returns status=delivered")
    check("mail_uid" in r, "send mints a mail_uid")
    mid = r["message_id"]; created.append(mid)

    # 2. List shows it as NEW.
    r = mail("list", "--project", PROJECT)
    check(r["messages"], "list returns the sent row")
    row = next(m for m in r["messages"] if m["message_id"] == mid)
    check(row["status"] == "NEW", "listed row is NEW")

    # 3. Get is side-effect free.
    r = mail("get", mid)
    check(r["message"]["status"] == "NEW", "get leaves status NEW")

    # 4. Read transitions NEW → READ.
    r = mail("read", mid)
    check(r["status"] == "READ", "read transitions to READ")

    # 5. Attention routing: register alias, send to {claude, none, '*'}.
    mail("register", "--alias", ALIAS, "--project", PROJECT)
    a = mail("send", "--to-host", HOST, "--to-project", PROJECT,
             "--attention", ALIAS, "--subject", "for-claude")["message_id"]
    n = mail("send", "--to-host", HOST, "--to-project", PROJECT,
             "--subject", "household")["message_id"]
    s = mail("send", "--to-host", HOST, "--to-project", PROJECT,
             "--attention", "*", "--subject", "broadcast")["message_id"]
    created += [a, n, s]

    seen = mail("list", "--alias", ALIAS, "--project", PROJECT, "--status", "NEW")
    ids = {m["message_id"] for m in seen["messages"]}
    check({a, n, s} <= ids, "alias 'claude' sees claude + household + broadcast")

    other = mail("list", "--alias", "nobody", "--project", PROJECT, "--status", "NEW")
    oids = {m["message_id"] for m in other["messages"]}
    check(a not in oids, "other alias does NOT see claude-addressed mail")
    check({n, s} <= oids, "other alias still sees household + broadcast")

    # 6. Bad JSON payload → exit 2 before any network call.
    mail("send", "--to-host", HOST, "--to-project", PROJECT,
         "--payload", "{not json}", expect_code=2)
    check(True, "bad --payload rejected with exit 2")

    # 7. Cross-node spool (optional, needs a second host).
    peer = os.environ.get("PEER_HOST")
    if peer:
        r = mail("send", "--to-host", peer, "--to-project", PROJECT, "--subject", "xnode")
        check(r["status"] == "queued", f"cross-node send to {peer} spools as queued")
        ob = mail("outbox", "--status", "QUEUED", "--status", "DELIVERED")
        check(any(row["to_host"] == peer for row in ob["rows"]),
              "outbox shows the cross-node row")
        print(f"  NOTE: verify delivery manually on {peer}: stele mail list --project {PROJECT}")
    else:
        print("  skip: cross-node (set PEER_HOST=<host> to exercise spool/deliver)")

    print("PASS: stele mail smoke complete")


def cleanup():
    for mid in created:
        subprocess.run(["stele", "mail", "archive", str(mid)],
                       capture_output=True, text=True, env=env)


if __name__ == "__main__":
    try:
        main()
    except AssertionError as e:
        print(e, file=sys.stderr)
        sys.exit(1)
    finally:
        cleanup()
