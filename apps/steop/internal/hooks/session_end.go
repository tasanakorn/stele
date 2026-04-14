package hooks

import (
	"context"
	"encoding/json"

	"github.com/tasanakorn/stele/apps/steop/internal/client"
	"github.com/tasanakorn/stele/apps/steop/internal/logging"
	"github.com/tasanakorn/stele/apps/steop/internal/store"
)

// HandleSessionEnd logs a session_end event and best-effort posts a session
// summary to the inbox endpoint. Always returns Allow().
func HandleSessionEnd(in *HookInput, db *store.DB, c *client.Client) []byte {
	if in == nil || c == nil || db == nil || in.SessionID == "" {
		return Allow()
	}
	id, ok := sessionIdent(c, in.SessionID)
	if !ok {
		return Allow()
	}
	sid := c.SessionCompositeID(in.SessionID)
	ctx := context.Background()

	logPayload, _ := json.Marshal(map[string]interface{}{
		"reason":          in.Reason,
		"cwd":             in.Cwd,
		"transcript_path": in.TranscriptPath,
	})
	if _, err := db.LogAppend(ctx, id, "session_end", logPayload); err != nil {
		logging.Debugf("session_end log failed: %v", err)
	}

	state, stateErr := db.StateGet(ctx, id)
	if stateErr != nil {
		logging.Debugf("session_end state get failed: %v", stateErr)
	}

	payload := map[string]interface{}{
		"cwd":             in.Cwd,
		"reason":          in.Reason,
		"transcript_path": in.TranscriptPath,
	}
	if c.ProjectDirResolved() {
		payload["resolved_project_dir"] = true
	}
	if state != nil {
		var data, counters any
		if len(state.Data) > 0 {
			_ = json.Unmarshal(state.Data, &data)
		}
		if len(state.Counters) > 0 {
			_ = json.Unmarshal(state.Counters, &counters)
		}
		payload["data"] = data
		payload["counters"] = counters
	}
	subject := in.Reason
	if subject == "" {
		subject = "session ended"
	}
	if _, err := c.MailboxSend(sid, c.ProjectID(), client.MailboxSendOptions{
		MessageType: "HOOK:SessionEnd",
		Subject:     subject,
		Payload:     payload,
	}); err != nil {
		logging.Debugf("session_end mailbox send failed: %v", err)
	}
	cleanupWatcherTasks(db, c, id, sid)
	if _, err := db.SessionStop(ctx, id); err != nil {
		logging.Debugf("session_end session stop failed: %v", err)
	}
	return Allow()
}
