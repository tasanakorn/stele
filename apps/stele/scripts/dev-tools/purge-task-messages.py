#!/usr/bin/env python3
"""
purge-task-messages.py — Delete ALL TASK:* messages from the Stele mailbox.

Usage:
    python purge-task-messages.py [--db PATH] [--dry-run] [--yes] [--status STATUS]

Options:
    --db PATH       Path to stele.db. Defaults to STELE_DB env var, then
                    ~/Library/Application Support/Stele/stele.db (macOS) or
                    ~/.local/share/Stele/stele.db (Linux).
    --dry-run       Show what would be deleted without deleting anything.
    --yes           Skip confirmation prompt.
    --status STATUS Comma-separated statuses to delete: NEW,READ,ARCHIVE (default: all).
"""

import argparse
import os
import platform
import sqlite3
import sys
from pathlib import Path


def default_db_path() -> Path:
    system = platform.system()
    if system == "Darwin":
        base = Path.home() / "Library" / "Application Support" / "Stele"
    elif system == "Linux":
        base = Path(os.environ.get("XDG_DATA_HOME", Path.home() / ".local" / "share")) / "Stele"
    else:
        base = Path.home() / ".local" / "share" / "Stele"
    return base / "stele.db"


def resolve_db(arg_db: str | None) -> Path:
    if arg_db:
        return Path(arg_db)
    env = os.environ.get("STELE_DB")
    if env:
        return Path(env)
    return default_db_path()


def main() -> int:
    ap = argparse.ArgumentParser(description="Delete ALL TASK:* messages from the Stele mailbox.")
    ap.add_argument("--db", metavar="PATH", help="Path to stele.db")
    ap.add_argument("--dry-run", action="store_true", help="Show what would be deleted; make no changes.")
    ap.add_argument("--yes", "-y", action="store_true", help="Skip confirmation prompt.")
    ap.add_argument("--status", metavar="STATUS", default="NEW,READ,ARCHIVE",
                    help="Comma-separated statuses to target (default: NEW,READ,ARCHIVE).")
    args = ap.parse_args()

    db_path = resolve_db(args.db)
    if not db_path.exists():
        print(f"error: database not found: {db_path}", file=sys.stderr)
        print("hint:  pass --db PATH or set STELE_DB", file=sys.stderr)
        return 1

    print(f"database: {db_path}")

    statuses = [s.strip().upper() for s in args.status.split(",") if s.strip()]
    placeholders = ",".join("?" for _ in statuses)

    conn = sqlite3.connect(str(db_path))
    conn.row_factory = sqlite3.Row

    rows = conn.execute(
        f"SELECT message_id, from_id, to_id, subject, message_type, status, created_at "
        f"FROM steop_mailbox WHERE message_type LIKE 'TASK:%' AND status IN ({placeholders}) "
        f"ORDER BY created_at ASC",
        statuses,
    ).fetchall()

    if not rows:
        print("no TASK:* messages found.")
        return 0

    print(f"\nfound {len(rows)} TASK:* message(s):\n")
    for row in rows:
        ts = row["created_at"][:19]
        print(f"  [{row['message_id']:>6}] {ts}  {row['message_type']:<18} {row['status']:<8}")
        print(f"           from: {row['from_id']}")
        print(f"             to: {row['to_id']}")
        print(f"        subject: {row['subject'] or '(none)'}")
        print()

    if args.dry_run:
        print("[dry-run] no changes made.")
        return 0

    if not args.yes:
        ans = input(f"delete {len(rows)} row(s)? [y/N] ").strip().lower()
        if ans != "y":
            print("aborted.")
            return 0

    ids = [row["message_id"] for row in rows]
    ph = ",".join("?" for _ in ids)
    cur = conn.execute(f"DELETE FROM steop_mailbox WHERE message_id IN ({ph})", ids)
    conn.commit()
    print(f"deleted {cur.rowcount} row(s).")
    return 0


if __name__ == "__main__":
    sys.exit(main())
