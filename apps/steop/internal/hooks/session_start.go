package hooks

import (
	"github.com/tasanakorn/stele/apps/steop/internal/client"
	"github.com/tasanakorn/stele/apps/steop/internal/logging"
)

// HandleSessionStart registers the session and logs a session_start event. Always returns Allow().
func HandleSessionStart(in *HookInput, c *client.Client) []byte {
	if in == nil || c == nil {
		return Allow()
	}
	sid := c.SessionCompositeID(in.SessionID)
	startData := map[string]interface{}{
		"cwd":             in.Cwd,
		"permission_mode": in.PermissionMode,
	}
	if c.ProjectDirResolved() {
		startData["resolved_project_dir"] = true
	}
	if _, err := c.SessionStart(sid, startData); err != nil {
		logging.Debugf("session_start session register failed: %v", err)
	}
	ev := client.LogEvent{
		ID:    sid,
		Event: "session_start",
		Data: map[string]interface{}{
			"cwd":             in.Cwd,
			"permission_mode": in.PermissionMode,
		},
	}
	if err := c.Log(ev); err != nil {
		logging.Debugf("session_start log failed: %v", err)
	}
	return Allow()
}
