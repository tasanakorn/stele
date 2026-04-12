package hooks

import (
	"github.com/tasanakorn/stele/apps/steop/internal/client"
	"github.com/tasanakorn/stele/apps/steop/internal/logging"
)

// HandlePreCompact logs a pre_compact event. Always returns Allow().
func HandlePreCompact(in *HookInput, c *client.Client) []byte {
	if in == nil || c == nil {
		return Allow()
	}
	ev := client.LogEvent{
		ID:    c.SessionCompositeID(in.SessionID),
		Event: "pre_compact",
		Data: map[string]interface{}{
			"trigger": in.Trigger,
			"cwd":     in.Cwd,
		},
	}
	if err := c.Log(ev); err != nil {
		logging.Debugf("pre_compact log failed: %v", err)
	}
	return Allow()
}
