package store

import (
	"context"
	"encoding/json"
	"time"
)

// Status is the statusline projection for a session.
type Status struct {
	Host         string
	ProjectDir   string
	SessionID    string
	Mode         string
	Phase        string
	Step         string
	ToolCalls    int64
	LoopCount    int64
	StepRetry    int64
	LastActiveAt int64
}

// StateGet returns the session row (alias of SessionGet for semantic clarity).
func (d *DB) StateGet(ctx context.Context, id Identity) (*Session, error) {
	return d.SessionGet(ctx, id)
}

// StatePut upserts data on the session. If merge is true the patch is
// shallow-merged into the existing data column; otherwise data is replaced.
func (d *DB) StatePut(ctx context.Context, id Identity, data json.RawMessage, merge bool) (*Session, error) {
	if err := requireSession(id); err != nil {
		return nil, err
	}
	now := time.Now().Unix()
	tx, err := d.BeginImmediate(ctx)
	if err != nil {
		return nil, err
	}
	defer tx.Rollback()
	if err := ensureSessionTx(ctx, tx, id, now); err != nil {
		return nil, err
	}
	var next string
	if merge {
		var existing string
		if err := tx.QueryRowContext(ctx,
			`SELECT data FROM steop_sessions WHERE host=? AND project_dir=? AND session_id=?`,
			id.Host, id.ProjectDir, id.SessionID,
		).Scan(&existing); err != nil {
			return nil, err
		}
		next, err = mergeJSON(existing, data)
		if err != nil {
			return nil, err
		}
	} else {
		if len(data) == 0 {
			next = "{}"
		} else {
			next = string(data)
		}
	}
	if _, err := tx.ExecContext(ctx,
		`UPDATE steop_sessions SET data=?, last_active_at=?
		 WHERE host=? AND project_dir=? AND session_id=?`,
		next, now, id.Host, id.ProjectDir, id.SessionID,
	); err != nil {
		return nil, err
	}
	if err := tx.Commit(); err != nil {
		return nil, err
	}
	return d.SessionGet(ctx, id)
}

// StateIncr atomically bumps a named counter and returns the new value.
func (d *DB) StateIncr(ctx context.Context, id Identity, counter string, delta int64) (int64, error) {
	if err := requireSession(id); err != nil {
		return 0, err
	}
	now := time.Now().Unix()
	tx, err := d.BeginImmediate(ctx)
	if err != nil {
		return 0, err
	}
	defer tx.Rollback()
	if err := ensureSessionTx(ctx, tx, id, now); err != nil {
		return 0, err
	}
	var raw string
	if err := tx.QueryRowContext(ctx,
		`SELECT counters FROM steop_sessions WHERE host=? AND project_dir=? AND session_id=?`,
		id.Host, id.ProjectDir, id.SessionID,
	).Scan(&raw); err != nil {
		return 0, err
	}
	counters := map[string]int64{}
	if raw != "" && raw != "null" {
		_ = json.Unmarshal([]byte(raw), &counters)
	}
	counters[counter] += delta
	newVal := counters[counter]
	encoded, err := json.Marshal(counters)
	if err != nil {
		return 0, err
	}
	if _, err := tx.ExecContext(ctx,
		`UPDATE steop_sessions SET counters=?, last_active_at=?
		 WHERE host=? AND project_dir=? AND session_id=?`,
		string(encoded), now, id.Host, id.ProjectDir, id.SessionID,
	); err != nil {
		return 0, err
	}
	if err := tx.Commit(); err != nil {
		return 0, err
	}
	return newVal, nil
}

// StateReset sets a counter to the given value (creating the session if
// needed) and returns it.
func (d *DB) StateReset(ctx context.Context, id Identity, counter string, value int64) (int64, error) {
	if err := requireSession(id); err != nil {
		return 0, err
	}
	now := time.Now().Unix()
	tx, err := d.BeginImmediate(ctx)
	if err != nil {
		return 0, err
	}
	defer tx.Rollback()
	if err := ensureSessionTx(ctx, tx, id, now); err != nil {
		return 0, err
	}
	var raw string
	if err := tx.QueryRowContext(ctx,
		`SELECT counters FROM steop_sessions WHERE host=? AND project_dir=? AND session_id=?`,
		id.Host, id.ProjectDir, id.SessionID,
	).Scan(&raw); err != nil {
		return 0, err
	}
	counters := map[string]int64{}
	if raw != "" && raw != "null" {
		_ = json.Unmarshal([]byte(raw), &counters)
	}
	counters[counter] = value
	encoded, err := json.Marshal(counters)
	if err != nil {
		return 0, err
	}
	if _, err := tx.ExecContext(ctx,
		`UPDATE steop_sessions SET counters=?, last_active_at=?
		 WHERE host=? AND project_dir=? AND session_id=?`,
		string(encoded), now, id.Host, id.ProjectDir, id.SessionID,
	); err != nil {
		return 0, err
	}
	if err := tx.Commit(); err != nil {
		return 0, err
	}
	return value, nil
}

// StateDelete removes a session row. Returns true if a row was deleted.
func (d *DB) StateDelete(ctx context.Context, id Identity) (bool, error) {
	if err := requireSession(id); err != nil {
		return false, err
	}
	res, err := d.sql.ExecContext(ctx,
		`DELETE FROM steop_sessions WHERE host=? AND project_dir=? AND session_id=?`,
		id.Host, id.ProjectDir, id.SessionID,
	)
	if err != nil {
		return false, err
	}
	n, _ := res.RowsAffected()
	return n > 0, nil
}

// StatusGet returns the statusline projection. Never returns a "not found"
// error — missing sessions yield zero values.
func (d *DB) StatusGet(ctx context.Context, id Identity) (*Status, error) {
	if err := requireSession(id); err != nil {
		return nil, err
	}
	status := &Status{
		Host: id.Host, ProjectDir: id.ProjectDir, SessionID: id.SessionID,
	}
	sess, err := d.SessionGet(ctx, id)
	if err != nil {
		return nil, err
	}
	if sess == nil {
		return status, nil
	}
	status.LastActiveAt = sess.LastActiveAt
	var data map[string]any
	if len(sess.Data) > 0 {
		_ = json.Unmarshal(sess.Data, &data)
	}
	var counters map[string]int64
	if len(sess.Counters) > 0 {
		_ = json.Unmarshal(sess.Counters, &counters)
	}
	if s, ok := data["mode"].(string); ok {
		status.Mode = s
	}
	if s, ok := data["phase"].(string); ok {
		status.Phase = s
	}
	if s, ok := data["step"].(string); ok {
		status.Step = s
	}
	status.ToolCalls = counters["tool_calls"]
	status.LoopCount = counters["loop_count"]
	status.StepRetry = counters["step_retry"]
	return status, nil
}
