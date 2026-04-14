package hooks

import (
	"context"
	"encoding/json"

	"github.com/tasanakorn/stele/apps/steop/internal/client"
	"github.com/tasanakorn/stele/apps/steop/internal/logging"
	"github.com/tasanakorn/stele/apps/steop/internal/store"
)

// HandleSubagentStop logs a subagent_stop event. Always returns Allow().
func HandleSubagentStop(in *HookInput, db *store.DB, c *client.Client) []byte {
	if in == nil || db == nil || c == nil {
		return Allow()
	}
	id, ok := sessionIdent(c, in.SessionID)
	if !ok {
		return Allow()
	}
	success := true
	if in.Success != nil {
		success = *in.Success
	}
	payload, _ := json.Marshal(map[string]interface{}{
		"agent_id":   in.AgentID,
		"agent_type": in.AgentType,
		"output":     truncateRunes(in.Output, 500),
		"success":    success,
	})
	if _, err := db.LogAppend(context.Background(), id, "subagent_stop", payload); err != nil {
		logging.Debugf("subagent_stop log failed: %v", err)
	}
	return Allow()
}
