package hooks

import (
	"github.com/tasanakorn/stele/apps/steop/internal/client"
	"github.com/tasanakorn/stele/apps/steop/internal/logging"
)

// HandleSessionStart logs a session_start event. Always returns Allow().
func HandleSessionStart(in *HookInput, c *client.Client) []byte {
	if in == nil || c == nil {
		return Allow()
	}
	ev := client.LogEvent{
		SessionID: in.SessionID,
		Event:     "session_start",
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
