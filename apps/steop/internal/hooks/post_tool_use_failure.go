package hooks

import (
	"context"
	"encoding/json"

	"github.com/tasanakorn/stele/apps/steop/internal/client"
	"github.com/tasanakorn/stele/apps/steop/internal/logging"
	"github.com/tasanakorn/stele/apps/steop/internal/store"
)

// HandlePostToolUseFailure logs a post_tool_use_failure event. Always returns Allow().
func HandlePostToolUseFailure(in *HookInput, db *store.DB, c *client.Client) []byte {
	if in == nil || db == nil || c == nil {
		return Allow()
	}
	id, ok := sessionIdent(c, in.SessionID)
	if !ok {
		return Allow()
	}
	payload, _ := json.Marshal(map[string]interface{}{
		"tool_name":    in.ToolName,
		"error":        in.Error,
		"is_interrupt": in.IsInterrupt,
	})
	if _, err := db.LogAppend(context.Background(), id, "post_tool_use_failure", payload); err != nil {
		logging.Debugf("post_tool_use_failure log failed: %v", err)
	}
	return Allow()
}
