package hooks

import (
	"context"
	"encoding/json"

	"github.com/tasanakorn/stele/apps/steop/internal/client"
	"github.com/tasanakorn/stele/apps/steop/internal/logging"
	"github.com/tasanakorn/stele/apps/steop/internal/store"
)

// HandleSessionStart registers the session and logs a session_start event. Always returns Allow().
func HandleSessionStart(in *HookInput, db *store.DB, c *client.Client) []byte {
	if in == nil || db == nil || c == nil {
		return Allow()
	}
	id, ok := sessionIdent(c, in.SessionID)
	if !ok {
		return Allow()
	}
	ctx := context.Background()
	startData := map[string]interface{}{
		"cwd":             in.Cwd,
		"permission_mode": in.PermissionMode,
	}
	if c.ProjectDirResolved() {
		startData["resolved_project_dir"] = true
	}
	payload, err := json.Marshal(startData)
	if err != nil {
		logging.Debugf("session_start marshal data: %v", err)
		return Allow()
	}
	if _, err := db.SessionStart(ctx, id, payload); err != nil {
		logging.Debugf("session_start session register failed: %v", err)
	}
	logPayload, _ := json.Marshal(map[string]interface{}{
		"cwd":             in.Cwd,
		"permission_mode": in.PermissionMode,
	})
	if _, err := db.LogAppend(ctx, id, "session_start", logPayload); err != nil {
		logging.Debugf("session_start log failed: %v", err)
	}
	return Allow()
}
