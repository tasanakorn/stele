package store

import (
	"context"
	"database/sql"
	"encoding/json"
	"fmt"
	"time"
)

// LogRow is one row in steop_logs. SessionID is "" for project-level events.
type LogRow struct {
	ID         int64
	Host       string
	ProjectDir string
	SessionID  string
	Event      string
	Payload    json.RawMessage
	CreatedAt  int64
}

// LogAppend inserts a log row and returns its autoincrement id.
func (d *DB) LogAppend(ctx context.Context, id Identity, event string, payload json.RawMessage) (int64, error) {
	now := time.Now().Unix()
	payloadStr := "{}"
	if len(payload) > 0 {
		payloadStr = string(payload)
	}
	var res sql.Result
	var err error
	if id.IsProject() {
		res, err = d.sql.ExecContext(ctx,
			`INSERT INTO steop_logs (host, project_dir, session_id, event, payload, created_at)
			 VALUES (?, ?, NULL, ?, ?, ?)`,
			id.Host, id.ProjectDir, event, payloadStr, now,
		)
	} else {
		res, err = d.sql.ExecContext(ctx,
			`INSERT INTO steop_logs (host, project_dir, session_id, event, payload, created_at)
			 VALUES (?, ?, ?, ?, ?, ?)`,
			id.Host, id.ProjectDir, id.SessionID, event, payloadStr, now,
		)
	}
	if err != nil {
		return 0, err
	}
	return res.LastInsertId()
}

// LogQuery returns the most recent rows matching the filters, newest first.
// Any of host/projectDir/sessionID may be empty to skip that filter.
func (d *DB) LogQuery(ctx context.Context, host, projectDir, sessionID string, limit int) ([]LogRow, error) {
	if limit <= 0 {
		limit = 100
	}
	sqlStr := `SELECT id, host, project_dir, session_id, event, payload, created_at
	           FROM steop_logs WHERE 1=1`
	var args []any
	if host != "" {
		sqlStr += " AND host = ?"
		args = append(args, host)
	}
	if projectDir != "" {
		sqlStr += " AND project_dir = ?"
		args = append(args, projectDir)
	}
	if sessionID != "" {
		sqlStr += " AND session_id = ?"
		args = append(args, sessionID)
	}
	sqlStr += fmt.Sprintf(" ORDER BY id DESC LIMIT %d", limit)
	rows, err := d.sql.QueryContext(ctx, sqlStr, args...)
	if err != nil {
		return nil, err
	}
	defer rows.Close()
	var out []LogRow
	for rows.Next() {
		var r LogRow
		var sid sql.NullString
		var payload string
		if err := rows.Scan(&r.ID, &r.Host, &r.ProjectDir, &sid, &r.Event, &payload, &r.CreatedAt); err != nil {
			return nil, err
		}
		if sid.Valid {
			r.SessionID = sid.String
		}
		r.Payload = json.RawMessage(payload)
		out = append(out, r)
	}
	return out, rows.Err()
}
