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
	if _, err := c.SessionStart(c.Host(), c.ProjectDir(), in.SessionID, map[string]interface{}{
		"cwd":             in.Cwd,
		"permission_mode": in.PermissionMode,
	}); err != nil {
		logging.Debugf("session_start session register failed: %v", err)
	}
	ev := client.LogEvent{
		Host:       c.Host(),
		ProjectDir: c.ProjectDir(),
		SessionID:  in.SessionID,
		Event:      "session_start",
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
