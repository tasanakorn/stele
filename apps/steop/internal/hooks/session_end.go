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
	sid := c.SessionCompositeID(in.SessionID)
	if err := c.Log(client.LogEvent{
		ID:    sid,
		Event: "session_end",
		Data: map[string]interface{}{
			"reason":          in.Reason,
			"cwd":             in.Cwd,
			"transcript_path": in.TranscriptPath,
		},
	}); err != nil {
		logging.Debugf("session_end log failed: %v", err)
	}

	state, stateErr := c.StateGet(sid)
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
		payload["data"] = state.Data
		payload["counters"] = state.Counters
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
	if _, err := c.SessionStop(sid); err != nil {
		logging.Debugf("session_end session stop failed: %v", err)
	}
	return Allow()
}
