package hooks

import (
	"context"
	"encoding/json"
	"time"

	"github.com/tasanakorn/stele/apps/steop/internal/client"
	"github.com/tasanakorn/stele/apps/steop/internal/logging"
	"github.com/tasanakorn/stele/apps/steop/internal/store"
)

// HandlePostToolUse increments the tool_calls counter, updates session state
// with the last tool invocation, and appends a structured log event — all in a
// single BEGIN IMMEDIATE transaction per PRD-020 §4.5. Always returns Allow();
// store errors are logged and swallowed.
func HandlePostToolUse(in *HookInput, db *store.DB, c *client.Client) []byte {
	if in == nil || in.SessionID == "" || db == nil || c == nil {
		return Allow()
	}
	id := store.Identity{
		Host:       c.Host(),
		ProjectDir: c.ProjectDir(),
		SessionID:  in.SessionID,
	}
	if id.Host == "" || id.ProjectDir == "" || id.SessionID == "" {
		return Allow()
	}
	ctx := context.Background()
	if err := postToolUseTx(ctx, db, id, in.ToolName); err != nil {
		logging.Debugf("post_tool_use tx failed: %v", err)
	}
	return Allow()
}

// postToolUseTx does counter incr + state merge + log append atomically.
func postToolUseTx(ctx context.Context, db *store.DB, id store.Identity, toolName string) error {
	now := time.Now().Unix()
	tx, err := db.BeginImmediate(ctx)
	if err != nil {
		return err
	}
	defer tx.Rollback()

	if _, err := tx.ExecContext(ctx,
		`INSERT INTO steop_sessions
		 (host, project_dir, session_id, state, started_at, last_active_at)
		 VALUES (?, ?, ?, 'active', ?, ?)
		 ON CONFLICT(host, project_dir, session_id) DO NOTHING`,
		id.Host, id.ProjectDir, id.SessionID, now, now,
	); err != nil {
		return err
	}

	var countersRaw, dataRaw string
	if err := tx.QueryRowContext(ctx,
		`SELECT counters, data FROM steop_sessions
		 WHERE host=? AND project_dir=? AND session_id=?`,
		id.Host, id.ProjectDir, id.SessionID,
	).Scan(&countersRaw, &dataRaw); err != nil {
		return err
	}

	counters := map[string]int64{}
	if countersRaw != "" && countersRaw != "null" {
		_ = json.Unmarshal([]byte(countersRaw), &counters)
	}
	counters["tool_calls"]++
	newCounters, err := json.Marshal(counters)
	if err != nil {
		return err
	}

	data := map[string]any{}
	if dataRaw != "" && dataRaw != "null" {
		_ = json.Unmarshal([]byte(dataRaw), &data)
	}
	data["last_tool"] = toolName
	data["last_tool_at"] = time.Now().UTC().Format(time.RFC3339)
	newData, err := json.Marshal(data)
	if err != nil {
		return err
	}

	if _, err := tx.ExecContext(ctx,
		`UPDATE steop_sessions SET counters=?, data=?, last_active_at=?
		 WHERE host=? AND project_dir=? AND session_id=?`,
		string(newCounters), string(newData), now,
		id.Host, id.ProjectDir, id.SessionID,
	); err != nil {
		return err
	}

	payload, err := json.Marshal(map[string]any{
		"tool_name": toolName,
		"ok":        true,
	})
	if err != nil {
		return err
	}
	if _, err := tx.ExecContext(ctx,
		`INSERT INTO steop_logs (host, project_dir, session_id, event, payload, created_at)
		 VALUES (?, ?, ?, ?, ?, ?)`,
		id.Host, id.ProjectDir, id.SessionID, "post_tool_use", string(payload), now,
	); err != nil {
		return err
	}

	return tx.Commit()
}
