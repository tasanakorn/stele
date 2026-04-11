package hooks

import (
	"github.com/tasanakorn/stele/apps/steop/internal/client"
	"github.com/tasanakorn/stele/apps/steop/internal/logging"
)

// HandlePostToolUseFailure logs a post_tool_use_failure event. Always returns Allow().
func HandlePostToolUseFailure(in *HookInput, c *client.Client) []byte {
	if in == nil || c == nil {
		return Allow()
	}
	ev := client.LogEvent{
		Host:       c.Host(),
		ProjectDir: c.ProjectDir(),
		SessionID:  in.SessionID,
		Event:      "post_tool_use_failure",
		Data: map[string]interface{}{
			"tool_name":    in.ToolName,
			"error":        in.Error,
			"is_interrupt": in.IsInterrupt,
		},
	}
	if err := c.Log(ev); err != nil {
		logging.Debugf("post_tool_use_failure log failed: %v", err)
	}
	return Allow()
}
