package store

import (
	"context"
	"database/sql"
	"encoding/json"
	"errors"
	"fmt"
	"time"
)

// Session is the typed projection of a steop_sessions row. Timestamps are
// unix seconds. Data and Counters stay as raw JSON so callers can decode
// whatever shape they need without forcing a schema here.
type Session struct {
	Host         string
	ProjectDir   string
	SessionID    string
	State        string
	StartedAt    int64
	LastActiveAt int64
	StoppedAt    *int64
	Data         json.RawMessage
	Counters     json.RawMessage
}

// Project is the 2-segment (host, project_dir) pair returned by ProjectList.
type Project struct {
	Host       string
	ProjectDir string
}

// requireSession returns an error if id is project-level. Most session ops
// require the full 3-segment id.
func requireSession(id Identity) error {
	if id.SessionID == "" {
		return errors.New("operation requires a 3-segment session id")
	}
	return nil
}

// SessionStart creates or reactivates a session row. If data is non-nil the
// JSON is shallow-merged into the existing data column.
func (d *DB) SessionStart(ctx context.Context, id Identity, data json.RawMessage) (*Session, error) {
	if err := requireSession(id); err != nil {
		return nil, err
	}
	now := time.Now().Unix()
	tx, err := d.BeginImmediate(ctx)
	if err != nil {
		return nil, err
	}
	defer tx.Rollback()

	var existing string
	err = tx.QueryRowContext(ctx,
		`SELECT data FROM steop_sessions WHERE host=? AND project_dir=? AND session_id=?`,
		id.Host, id.ProjectDir, id.SessionID,
	).Scan(&existing)
	switch {
	case err == sql.ErrNoRows:
		payload := []byte("{}")
		if len(data) > 0 {
			payload = data
		}
		if _, err := tx.ExecContext(ctx,
			`INSERT INTO steop_sessions
			 (host, project_dir, session_id, state, started_at, last_active_at, data)
			 VALUES (?, ?, ?, 'active', ?, ?, ?)`,
			id.Host, id.ProjectDir, id.SessionID, now, now, string(payload),
		); err != nil {
			return nil, err
		}
	case err != nil:
		return nil, err
	default:
		merged := existing
		if len(data) > 0 {
			m, err := mergeJSON(existing, data)
			if err != nil {
				return nil, err
			}
			merged = m
		}
		if _, err := tx.ExecContext(ctx,
			`UPDATE steop_sessions SET state='active', last_active_at=?, stopped_at=NULL, data=?
			 WHERE host=? AND project_dir=? AND session_id=?`,
			now, merged, id.Host, id.ProjectDir, id.SessionID,
		); err != nil {
			return nil, err
		}
	}
	if err := tx.Commit(); err != nil {
		return nil, err
	}
	return d.SessionGet(ctx, id)
}

// SessionStop marks the session stopped.
func (d *DB) SessionStop(ctx context.Context, id Identity) (*Session, error) {
	if err := requireSession(id); err != nil {
		return nil, err
	}
	now := time.Now().Unix()
	res, err := d.sql.ExecContext(ctx,
		`UPDATE steop_sessions SET state='stopped', stopped_at=?, last_active_at=?
		 WHERE host=? AND project_dir=? AND session_id=?`,
		now, now, id.Host, id.ProjectDir, id.SessionID,
	)
	if err != nil {
		return nil, err
	}
	n, _ := res.RowsAffected()
	if n == 0 {
		// Ensure-then-stop so callers can stop a session they never explicitly
		// started (matches stele-server behaviour).
		if _, err := d.sql.ExecContext(ctx,
			`INSERT INTO steop_sessions
			 (host, project_dir, session_id, state, started_at, last_active_at, stopped_at)
			 VALUES (?, ?, ?, 'stopped', ?, ?, ?)`,
			id.Host, id.ProjectDir, id.SessionID, now, now, now,
		); err != nil {
			return nil, err
		}
	}
	return d.SessionGet(ctx, id)
}

// SessionTouch refreshes last_active_at.
func (d *DB) SessionTouch(ctx context.Context, id Identity) (*Session, error) {
	if err := requireSession(id); err != nil {
		return nil, err
	}
	now := time.Now().Unix()
	if err := d.ensureSession(ctx, id, now); err != nil {
		return nil, err
	}
	if _, err := d.sql.ExecContext(ctx,
		`UPDATE steop_sessions SET last_active_at=?
		 WHERE host=? AND project_dir=? AND session_id=?`,
		now, id.Host, id.ProjectDir, id.SessionID,
	); err != nil {
		return nil, err
	}
	return d.SessionGet(ctx, id)
}

// SessionGet returns the session row or (nil, nil) if it doesn't exist.
func (d *DB) SessionGet(ctx context.Context, id Identity) (*Session, error) {
	if err := requireSession(id); err != nil {
		return nil, err
	}
	row := d.sql.QueryRowContext(ctx,
		`SELECT host, project_dir, session_id, state, started_at, last_active_at,
		        stopped_at, data, counters
		 FROM steop_sessions WHERE host=? AND project_dir=? AND session_id=?`,
		id.Host, id.ProjectDir, id.SessionID,
	)
	return scanSession(row)
}

// SessionList lists sessions matching the optional filters. An empty host /
// projectDir / state means "any".
func (d *DB) SessionList(ctx context.Context, host, projectDir, state string, limit int) ([]Session, error) {
	sql := `SELECT host, project_dir, session_id, state, started_at, last_active_at,
	               stopped_at, data, counters
	        FROM steop_sessions WHERE 1=1`
	var args []any
	if host != "" {
		sql += " AND host = ?"
		args = append(args, host)
	}
	if projectDir != "" {
		sql += " AND project_dir = ?"
		args = append(args, projectDir)
	}
	if state != "" {
		sql += " AND state = ?"
		args = append(args, state)
	}
	sql += " ORDER BY last_active_at DESC"
	if limit > 0 {
		sql += fmt.Sprintf(" LIMIT %d", limit)
	}
	rows, err := d.sql.QueryContext(ctx, sql, args...)
	if err != nil {
		return nil, err
	}
	defer rows.Close()
	var out []Session
	for rows.Next() {
		s, err := scanSessionRows(rows)
		if err != nil {
			return nil, err
		}
		out = append(out, *s)
	}
	return out, rows.Err()
}

// ProjectList lists distinct (host, project_dir) pairs in steop_sessions.
// An empty host includes every host.
func (d *DB) ProjectList(ctx context.Context, host string) ([]Project, error) {
	var rows *sql.Rows
	var err error
	if host != "" {
		rows, err = d.sql.QueryContext(ctx,
			`SELECT DISTINCT host, project_dir FROM steop_sessions
			 WHERE host=? ORDER BY host, project_dir`, host)
	} else {
		rows, err = d.sql.QueryContext(ctx,
			`SELECT DISTINCT host, project_dir FROM steop_sessions
			 ORDER BY host, project_dir`)
	}
	if err != nil {
		return nil, err
	}
	defer rows.Close()
	var out []Project
	for rows.Next() {
		var p Project
		if err := rows.Scan(&p.Host, &p.ProjectDir); err != nil {
			return nil, err
		}
		out = append(out, p)
	}
	return out, rows.Err()
}

// ensureSession inserts an empty session row if none exists. Used by
// state/incr/touch paths that create-on-write.
func (d *DB) ensureSession(ctx context.Context, id Identity, now int64) error {
	_, err := d.sql.ExecContext(ctx,
		`INSERT INTO steop_sessions
		 (host, project_dir, session_id, state, started_at, last_active_at)
		 VALUES (?, ?, ?, 'active', ?, ?)
		 ON CONFLICT(host, project_dir, session_id) DO NOTHING`,
		id.Host, id.ProjectDir, id.SessionID, now, now,
	)
	return err
}

// ensureSessionTx is the in-transaction variant.
func ensureSessionTx(ctx context.Context, tx *Tx, id Identity, now int64) error {
	_, err := tx.ExecContext(ctx,
		`INSERT INTO steop_sessions
		 (host, project_dir, session_id, state, started_at, last_active_at)
		 VALUES (?, ?, ?, 'active', ?, ?)
		 ON CONFLICT(host, project_dir, session_id) DO NOTHING`,
		id.Host, id.ProjectDir, id.SessionID, now, now,
	)
	return err
}

type scanner interface {
	Scan(dest ...any) error
}

func scanSession(s scanner) (*Session, error) {
	var sess Session
	var data, counters string
	var stopped sql.NullInt64
	err := s.Scan(
		&sess.Host, &sess.ProjectDir, &sess.SessionID,
		&sess.State, &sess.StartedAt, &sess.LastActiveAt,
		&stopped, &data, &counters,
	)
	if err == sql.ErrNoRows {
		return nil, nil
	}
	if err != nil {
		return nil, err
	}
	if stopped.Valid {
		v := stopped.Int64
		sess.StoppedAt = &v
	}
	sess.Data = json.RawMessage(data)
	sess.Counters = json.RawMessage(counters)
	return &sess, nil
}

func scanSessionRows(rows *sql.Rows) (*Session, error) {
	return scanSession(rows)
}

// mergeJSON shallow-merges patch into base and returns the serialized result.
func mergeJSON(base string, patch []byte) (string, error) {
	var b map[string]any
	if base == "" || base == "null" {
		b = map[string]any{}
	} else {
		if err := json.Unmarshal([]byte(base), &b); err != nil {
			return "", err
		}
	}
	var p map[string]any
	if err := json.Unmarshal(patch, &p); err != nil {
		return "", err
	}
	for k, v := range p {
		b[k] = v
	}
	out, err := json.Marshal(b)
	if err != nil {
		return "", err
	}
	return string(out), nil
}
