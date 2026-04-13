#!/usr/bin/env python3
"""
cleanup-dead-task-letters.py — Remove dead TASK:* mailbox entries from a Stele SQLite database.

Dead task letters are TASK:* messages in steop_mailbox whose recipient session
no longer exists or is stopped. They will never be claimed, so they accumulate
forever. This script identifies and optionally deletes them.

Usage:
    python cleanup-dead-task-letters.py [--db PATH] [--dry-run] [--yes] [--age-days N]

Options:
    --db PATH       Path to stele.db. Defaults to STELE_DB env var, then
                    ~/Library/Application Support/Stele/stele.db (macOS) or
                    ~/.local/share/Stele/stele.db (Linux).
    --dry-run       Show what would be deleted without deleting anything.
    --yes           Skip confirmation prompt.
    --age-days N    Only delete dead letters older than N days (default: 0,
                    meaning all dead letters regardless of age).
    --archived-days N
                    Also delete ARCHIVE status TASK:* messages older than N
                    days (default: off). Use 30 as a reasonable value.
"""

import argparse
import os
import platform
import sqlite3
import sys
from datetime import datetime, timedelta, timezone
from pathlib import Path


# ---------------------------------------------------------------------------
# DB path resolution
# ---------------------------------------------------------------------------

def default_db_path() -> Path:
    system = platform.system()
    if system == "Darwin":
        base = Path.home() / "Library" / "Application Support" / "Stele"
    elif system == "Linux":
        base = Path(os.environ.get("XDG_DATA_HOME", Path.home() / ".local" / "share")) / "Stele"
    elif system == "Windows":
        base = Path(os.environ.get("APPDATA", Path.home())) / "Stele"
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


# ---------------------------------------------------------------------------
# Queries
# ---------------------------------------------------------------------------

DEAD_LETTERS_QUERY = """
SELECT
    m.message_id,
    m.from_id,
    m.to_id,
    m.subject,
    m.message_type,
    m.status,
    m.created_at
FROM steop_mailbox m
WHERE m.message_type LIKE 'TASK:%'
  AND m.status IN ('NEW', 'READ')
  AND (
    -- 3-segment to_id: the session segment should be a UUID
    -- Recipient is a session-form id; check that session is stopped or missing
    (
        -- to_id has at least 2 colons  →  session-level id
        length(m.to_id) - length(replace(m.to_id, ':', '')) >= 2
        AND (
            -- session is explicitly stopped
            EXISTS (
                SELECT 1 FROM steop_sessions s
                WHERE s.session_id = substr(
                    m.to_id,
                    instr(m.to_id, ':') + instr(substr(m.to_id, instr(m.to_id, '|') + 1), ':') + 2
                )
                AND s.state = 'stopped'
            )
            OR
            -- session simply doesn't exist
            NOT EXISTS (
                SELECT 1 FROM steop_sessions s
                -- extract last colon-segment as session_id
                WHERE s.session_id = substr(m.to_id, (
                    -- position after second colon
                    instr(m.to_id, ':') + 1 +
                    instr(substr(m.to_id, instr(m.to_id, ':') + 1), ':')
                ))
            )
        )
    )
  )
  {age_filter}
ORDER BY m.created_at ASC;
"""

# Simpler query that uses Python-side filtering (more readable):
TASK_LETTERS_ALL = """
SELECT
    m.message_id,
    m.from_id,
    m.to_id,
    m.subject,
    m.message_type,
    m.status,
    m.created_at
FROM steop_mailbox m
WHERE m.message_type LIKE 'TASK:%'
  AND m.status IN ('NEW', 'READ')
ORDER BY m.created_at ASC;
"""

ARCHIVED_TASK_LETTERS = """
SELECT
    message_id,
    from_id,
    to_id,
    subject,
    message_type,
    status,
    created_at
FROM steop_mailbox
WHERE message_type LIKE 'TASK:%'
  AND status = 'ARCHIVE'
  AND created_at < :cutoff
ORDER BY created_at ASC;
"""

SESSIONS_ALL = """
SELECT host, project_dir, session_id, state, last_active_at, stopped_at
FROM steop_sessions;
"""


# ---------------------------------------------------------------------------
# Logic
# ---------------------------------------------------------------------------

def parse_dt(s: str) -> datetime:
    """Parse an ISO-8601 datetime string (with or without Z suffix)."""
    s = s.replace("Z", "+00:00")
    return datetime.fromisoformat(s)


def extract_session_id(composite_id: str) -> str | None:
    """
    Return the session_id segment from a 3-part composite id
    (host:project_dir:session_id), or None if it's a 2-part id.
    The project_dir itself may contain colons on Windows, but in practice
    the Stele identity grammar splits on the first colon only for host,
    then the second colon for the session segment.
    """
    parts = composite_id.split(":", 2)
    if len(parts) < 3:
        return None
    third = parts[2]
    # Literal "USER" is the user-level singleton, not a session
    if third == "USER":
        return None
    return third


def find_dead_letters(
    conn: sqlite3.Connection,
    age_days: int,
    archived_days: int | None,
) -> tuple[list[dict], list[dict]]:
    """
    Returns (dead_active, dead_archived).
    dead_active  — NEW/READ TASK:* messages whose recipient session is gone/stopped.
    dead_archived — ARCHIVE TASK:* messages older than archived_days (if set).
    """
    # Build a set of living session IDs
    sessions = conn.execute(SESSIONS_ALL).fetchall()
    live_sessions: set[str] = set()
    stopped_sessions: set[str] = set()
    for row in sessions:
        sid = row["session_id"]
        if row["state"] == "active":
            live_sessions.add(sid)
        else:
            stopped_sessions.add(sid)

    now = datetime.now(timezone.utc)
    age_cutoff = now - timedelta(days=age_days) if age_days > 0 else None

    # Check active/read task letters
    rows = conn.execute(TASK_LETTERS_ALL).fetchall()
    dead_active: list[dict] = []
    for row in rows:
        row = dict(row)
        sid = extract_session_id(row["to_id"])
        if sid is None:
            # Project-level or USER recipient — skip; those are always reachable
            continue
        if sid in live_sessions:
            continue
        # Recipient is stopped or missing
        row["dead_reason"] = (
            "recipient stopped" if sid in stopped_sessions else "recipient missing"
        )
        created = parse_dt(row["created_at"])
        if age_cutoff and created > age_cutoff:
            continue
        dead_active.append(row)

    # Check archived task letters
    dead_archived: list[dict] = []
    if archived_days is not None:
        cutoff = (now - timedelta(days=archived_days)).isoformat()
        archived_rows = conn.execute(ARCHIVED_TASK_LETTERS, {"cutoff": cutoff}).fetchall()
        for row in archived_rows:
            row = dict(row)
            row["dead_reason"] = f"archived >{archived_days}d ago"
            dead_archived.append(row)

    return dead_active, dead_archived


def fmt_row(row: dict) -> str:
    ts = row["created_at"][:19]
    return (
        f"  [{row['message_id']:>6}] {ts}  {row['message_type']:<18} "
        f"{row['status']:<8} {row['dead_reason']}\n"
        f"           from: {row['from_id']}\n"
        f"             to: {row['to_id']}\n"
        f"        subject: {row['subject'] or '(none)'}"
    )


def delete_rows(conn: sqlite3.Connection, ids: list[int]) -> int:
    if not ids:
        return 0
    placeholders = ",".join("?" for _ in ids)
    cur = conn.execute(
        f"DELETE FROM steop_mailbox WHERE message_id IN ({placeholders})", ids
    )
    conn.commit()
    return cur.rowcount


# ---------------------------------------------------------------------------
# CLI
# ---------------------------------------------------------------------------

def main() -> int:
    ap = argparse.ArgumentParser(
        description="Remove dead TASK:* mailbox entries from a Stele SQLite DB."
    )
    ap.add_argument("--db", metavar="PATH", help="Path to stele.db")
    ap.add_argument(
        "--dry-run",
        action="store_true",
        help="Show what would be deleted; make no changes.",
    )
    ap.add_argument(
        "--yes", "-y",
        action="store_true",
        help="Skip confirmation prompt.",
    )
    ap.add_argument(
        "--age-days",
        type=int,
        default=0,
        metavar="N",
        help="Only delete dead letters older than N days (default: 0 = all).",
    )
    ap.add_argument(
        "--archived-days",
        type=int,
        default=None,
        metavar="N",
        help="Also delete ARCHIVE TASK:* messages older than N days.",
    )
    args = ap.parse_args()

    db_path = resolve_db(args.db)
    if not db_path.exists():
        print(f"error: database not found: {db_path}", file=sys.stderr)
        print("hint:  pass --db PATH or set STELE_DB", file=sys.stderr)
        return 1

    print(f"database: {db_path}")

    conn = sqlite3.connect(str(db_path))
    conn.row_factory = sqlite3.Row

    dead_active, dead_archived = find_dead_letters(
        conn, args.age_days, args.archived_days
    )

    all_dead = dead_active + dead_archived
    if not all_dead:
        print("no dead task letters found.")
        return 0

    # Print summary
    print(f"\nfound {len(all_dead)} dead task letter(s):\n")

    if dead_active:
        print(f"  undeliverable NEW/READ ({len(dead_active)}):")
        for row in dead_active:
            print("  " + fmt_row(row))
            print()

    if dead_archived:
        print(f"  stale ARCHIVE ({len(dead_archived)}):")
        for row in dead_archived:
            print("  " + fmt_row(row))
            print()

    if args.dry_run:
        print("[dry-run] no changes made.")
        return 0

    if not args.yes:
        ans = input(f"delete {len(all_dead)} row(s)? [y/N] ").strip().lower()
        if ans != "y":
            print("aborted.")
            return 0

    ids = [row["message_id"] for row in all_dead]
    deleted = delete_rows(conn, ids)
    print(f"deleted {deleted} row(s).")
    return 0


if __name__ == "__main__":
    sys.exit(main())
