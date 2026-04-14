package hooks

import (
	"context"
	"encoding/json"

	"github.com/tasanakorn/stele/apps/steop/internal/client"
	"github.com/tasanakorn/stele/apps/steop/internal/logging"
	"github.com/tasanakorn/stele/apps/steop/internal/store"
)

// HandlePreCompact logs a pre_compact event. Always returns Allow().
func HandlePreCompact(in *HookInput, db *store.DB, c *client.Client) []byte {
	if in == nil || db == nil || c == nil {
		return Allow()
	}
	id, ok := sessionIdent(c, in.SessionID)
	if !ok {
		return Allow()
	}
	payload, _ := json.Marshal(map[string]interface{}{
		"trigger": in.Trigger,
		"cwd":     in.Cwd,
	})
	if _, err := db.LogAppend(context.Background(), id, "pre_compact", payload); err != nil {
		logging.Debugf("pre_compact log failed: %v", err)
	}
	return Allow()
}
