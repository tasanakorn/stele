package hooks

import (
	"github.com/tasanakorn/stele/apps/steop/internal/client"
	"github.com/tasanakorn/stele/apps/steop/internal/logging"
)

// HandleSubagentStop logs a subagent_stop event. Always returns Allow().
func HandleSubagentStop(in *HookInput, c *client.Client) []byte {
	if in == nil || c == nil {
		return Allow()
	}
	success := true
	if in.Success != nil {
		success = *in.Success
	}
	ev := client.LogEvent{
		Host:       c.Host(),
		ProjectDir: c.ProjectDir(),
		SessionID:  in.SessionID,
		Event:      "subagent_stop",
		Data: map[string]interface{}{
			"agent_id":   in.AgentID,
			"agent_type": in.AgentType,
			"output":     truncateRunes(in.Output, 500),
			"success":    success,
		},
	}
	if err := c.Log(ev); err != nil {
		logging.Debugf("subagent_stop log failed: %v", err)
	}
	return Allow()
}
