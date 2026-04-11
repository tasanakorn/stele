package hooks

import (
	"time"

	"github.com/tasanakorn/stele/apps/steop/internal/client"
	"github.com/tasanakorn/stele/apps/steop/internal/logging"
)

// HandlePostToolUse increments the tool_calls counter, updates session state
// with the last tool invocation, and emits a structured log event. Always
// returns Allow(); server errors are logged and swallowed.
func HandlePostToolUse(in *HookInput, c *client.Client) []byte {
	if in == nil || in.SessionID == "" || c == nil {
		return Allow()
	}
	if _, err := c.CounterIncr(c.Host(), c.ProjectDir(), in.SessionID, "tool_calls", 1); err != nil {
		logging.Debugf("post_tool_use counter incr failed: %v", err)
	}
	data := map[string]interface{}{
		"last_tool":    in.ToolName,
		"last_tool_at": time.Now().UTC().Format(time.RFC3339),
	}
	if _, err := c.StatePut(c.Host(), c.ProjectDir(), in.SessionID, data, true); err != nil {
		logging.Debugf("post_tool_use state put failed: %v", err)
	}
	if err := c.Log(client.LogEvent{
		Host:       c.Host(),
		ProjectDir: c.ProjectDir(),
		SessionID:  in.SessionID,
		Event:      "post_tool_use",
		Data: map[string]interface{}{
			"tool_name": in.ToolName,
			"ok":        true,
		},
	}); err != nil {
		logging.Debugf("post_tool_use log failed: %v", err)
	}
	return Allow()
}
