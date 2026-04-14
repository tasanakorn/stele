package hooks

import (
	"context"
	"encoding/json"

	"github.com/tasanakorn/stele/apps/steop/internal/client"
	"github.com/tasanakorn/stele/apps/steop/internal/logging"
	"github.com/tasanakorn/stele/apps/steop/internal/store"
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
func HandleSubagentStart(in *HookInput, db *store.DB, c *client.Client) []byte {
	if in == nil || db == nil || c == nil {
		return Allow()
	}
	id, ok := sessionIdent(c, in.SessionID)
	if !ok {
		return Allow()
	}
	payload, _ := json.Marshal(map[string]interface{}{
		"agent_id":   in.AgentID,
		"agent_type": in.AgentType,
		"model":      in.Model,
		"prompt":     truncateRunes(in.Prompt, 500),
	})
	if _, err := db.LogAppend(context.Background(), id, "subagent_start", payload); err != nil {
		logging.Debugf("subagent_start log failed: %v", err)
	}
	return Allow()
}
