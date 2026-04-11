package hooks

import (
	"github.com/tasanakorn/stele/apps/steop/internal/client"
	"github.com/tasanakorn/stele/apps/steop/internal/logging"
)

// HandleSessionEnd logs a session_end event and best-effort posts a session
// summary to the inbox endpoint. Always returns Allow().
func HandleSessionEnd(in *HookInput, c *client.Client) []byte {
	if in == nil || c == nil || in.SessionID == "" {
		return Allow()
	}
	if err := c.Log(client.LogEvent{
		SessionID: in.SessionID,
		Event:     "session_end",
		Data: map[string]interface{}{
			"reason":          in.Reason,
			"cwd":             in.Cwd,
			"transcript_path": in.TranscriptPath,
		},
	}); err != nil {
		logging.Debugf("session_end log failed: %v", err)
	}

	state, stateErr := c.StateGet(in.SessionID)
	if stateErr != nil {
		logging.Debugf("session_end state get failed: %v", stateErr)
	}

	payload := map[string]interface{}{
		"cwd":             in.Cwd,
		"reason":          in.Reason,
		"transcript_path": in.TranscriptPath,
	}
	if state != nil {
		payload["data"] = state.Data
		payload["counters"] = state.Counters
	}
	if err := c.Inbox(client.InboxEnvelope{SessionID: in.SessionID, Payload: payload}); err != nil {
		logging.Debugf("session_end inbox post failed: %v", err)
	}
	return Allow()
}
