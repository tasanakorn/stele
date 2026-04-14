package store

import (
	"context"
	"database/sql"
	"fmt"
)

// migrations is append-only. The slice index is the target user_version - 1.
// migrations[0] is the initial schema introduced at v0.16.0. Subsequent
// schema changes append — never reorder, never rewrite.
var migrations = []func(*sql.Tx) error{
	initialSchema,
}

// Migrate applies any pending migrations in a single transaction and bumps
// PRAGMA user_version atomically. Idempotent: a no-op once user_version
// matches len(migrations). Returns ErrSchemaNewer if the on-disk version is
// ahead of this binary.
func Migrate(db *DB) error {
	ctx := context.Background()
	var have int
	if err := db.sql.QueryRowContext(ctx, "PRAGMA user_version").Scan(&have); err != nil {
		return fmt.Errorf("read user_version: %w", err)
	}
	if have == len(migrations) {
		return nil
	}
	if have > len(migrations) {
		return errSchemaNewer
	}
	tx, err := db.sql.BeginTx(ctx, nil)
	if err != nil {
		return err
	}
	for i := have; i < len(migrations); i++ {
		if err := migrations[i](tx); err != nil {
			tx.Rollback()
			return fmt.Errorf("migration %d: %w", i, err)
		}
	}
	if _, err := tx.ExecContext(ctx, fmt.Sprintf("PRAGMA user_version = %d", len(migrations))); err != nil {
		tx.Rollback()
		return fmt.Errorf("bump user_version: %w", err)
	}
	return tx.Commit()
}

func initialSchema(tx *sql.Tx) error {
	stmts := []string{
		`CREATE TABLE IF NOT EXISTS steop_sessions (
			host          TEXT    NOT NULL,
			project_dir   TEXT    NOT NULL,
			session_id    TEXT    NOT NULL,
			state         TEXT    NOT NULL,
			started_at    INTEGER NOT NULL,
			last_active_at INTEGER NOT NULL,
			stopped_at    INTEGER,
			data          TEXT    NOT NULL DEFAULT '{}',
			counters      TEXT    NOT NULL DEFAULT '{}',
			PRIMARY KEY (host, project_dir, session_id)
		)`,
		`CREATE INDEX IF NOT EXISTS idx_steop_sessions_project
			ON steop_sessions(host, project_dir)`,
		`CREATE TABLE IF NOT EXISTS steop_storage_session (
			host         TEXT    NOT NULL,
			project_dir  TEXT    NOT NULL,
			session_id   TEXT    NOT NULL,
			key          TEXT    NOT NULL,
			content      TEXT    NOT NULL,
			created_at   INTEGER NOT NULL,
			updated_at   INTEGER NOT NULL,
			PRIMARY KEY (host, project_dir, session_id, key)
		)`,
		`CREATE TABLE IF NOT EXISTS steop_storage_project (
			host         TEXT    NOT NULL,
			project_dir  TEXT    NOT NULL,
			key          TEXT    NOT NULL,
			content      TEXT    NOT NULL,
			created_at   INTEGER NOT NULL,
			updated_at   INTEGER NOT NULL,
			PRIMARY KEY (host, project_dir, key)
		)`,
		`CREATE TABLE IF NOT EXISTS steop_logs (
			id           INTEGER PRIMARY KEY AUTOINCREMENT,
			host         TEXT    NOT NULL,
			project_dir  TEXT    NOT NULL,
			session_id   TEXT,
			event        TEXT    NOT NULL,
			payload      TEXT    NOT NULL DEFAULT '{}',
			created_at   INTEGER NOT NULL
		)`,
		`CREATE INDEX IF NOT EXISTS idx_steop_logs_session
			ON steop_logs(host, project_dir, session_id, id)`,
		`CREATE INDEX IF NOT EXISTS idx_steop_logs_project
			ON steop_logs(host, project_dir, id)`,
	}
	for _, s := range stmts {
		if _, err := tx.Exec(s); err != nil {
			return fmt.Errorf("exec %q: %w", firstLine(s), err)
		}
	}
	return nil
}

func firstLine(s string) string {
	for i, r := range s {
		if r == '\n' {
			return s[:i]
		}
	}
	return s
}
