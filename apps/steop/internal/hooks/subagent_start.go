package hooks

import (
	"github.com/tasanakorn/stele/apps/steop/internal/client"
	"github.com/tasanakorn/stele/apps/steop/internal/logging"
)

// truncateRunes returns s truncated to at most n runes.
func truncateRunes(s string, n int) string {
	r := []rune(s)
	if len(r) <= n {
		return s
	}
	return string(r[:n])
}

// HandleSubagentStart logs a subagent_start event. Always returns Allow().
func HandleSubagentStart(in *HookInput, c *client.Client) []byte {
	if in == nil || c == nil {
		return Allow()
	}
	ev := client.LogEvent{
		SessionID: in.SessionID,
		Event:     "subagent_start",
		Data: map[string]interface{}{
			"agent_id":   in.AgentID,
			"agent_type": in.AgentType,
			"model":      in.Model,
			"prompt":     truncateRunes(in.Prompt, 500),
		},
	}
	if err := c.Log(ev); err != nil {
		logging.Debugf("subagent_start log failed: %v", err)
	}
	return Allow()
}
