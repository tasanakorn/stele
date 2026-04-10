package hooks

import (
	"github.com/tasanakorn/stele/apps/steop/internal/client"
	"github.com/tasanakorn/stele/apps/steop/internal/logging"
)

// HandlePostToolUse increments the tool_calls counter for the session.
// Always returns Allow(); server errors are logged and swallowed.
func HandlePostToolUse(in *HookInput, c *client.Client) []byte {
	if in == nil || in.SessionID == "" || c == nil {
		return Allow()
	}
	if _, err := c.CounterIncr(in.SessionID, "tool_calls", 1); err != nil {
		logging.Debugf("post_tool_use counter incr failed: %v", err)
	}
	return Allow()
}
