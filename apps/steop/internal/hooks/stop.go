package hooks

import (
	"context"
	"encoding/json"
	"path/filepath"
	"strings"
	"time"

	"github.com/tasanakorn/stele/apps/steop/internal/client"
	"github.com/tasanakorn/stele/apps/steop/internal/logging"
	"github.com/tasanakorn/stele/apps/steop/internal/store"
)

const maxBodyLen = 140

// HandleStop fires a desktop notification via stele-server, posts a session
// summary to the inbox endpoint, and clears transient state for the session.
// Always returns Allow() to honor the non-blocking contract.
func HandleStop(in *HookInput, db *store.DB, c *client.Client) []byte {
	if in == nil || c == nil || db == nil {
		return Allow()
	}
	req := client.NotifyRequest{
		Title: defaultTitle(in.Cwd),
		Body:  buildBody(in.LastAssistantMessage),
	}
	if err := c.Notify(req); err != nil {
		logging.Debugf("notify call failed: %v", err)
	}
	if in.SessionID == "" {
		return Allow()
	}
	id, ok := sessionIdent(c, in.SessionID)
	if !ok {
		return Allow()
	}
	sid := c.SessionCompositeID(in.SessionID)
	ctx := context.Background()

	state, stateErr := db.StateGet(ctx, id)
	if stateErr != nil {
		logging.Debugf("stop state get failed: %v", stateErr)
	}

	if state != nil {
		var data map[string]interface{}
		var counters map[string]int64
		if len(state.Data) > 0 {
			_ = json.Unmarshal(state.Data, &data)
		}
		if len(state.Counters) > 0 {
			_ = json.Unmarshal(state.Counters, &counters)
		}
		payload := map[string]interface{}{
			"cwd":      in.Cwd,
			"data":     data,
			"counters": counters,
			"ended_at": time.Now().UTC().Format(time.RFC3339),
		}
		subject := buildBody(in.LastAssistantMessage)
		if _, err := c.MailboxSend(sid, c.ProjectID(), client.MailboxSendOptions{
			MessageType: "HOOK:Stop",
			Subject:     subject,
			Payload:     payload,
		}); err != nil {
			logging.Debugf("stop mailbox send failed: %v", err)
		}
		if pm, ok := data["persistent_mode"].(bool); ok && pm {
			logging.Debugf("persistent_mode set but not honored in v1")
		}
	}

	cleanupWatcherTasks(db, c, id, sid)

	if _, err := db.StorageDelete(ctx, id, "watcher:state"); err != nil {
		logging.Debugf("stop watcher:state cleanup: %v", err)
	}
	if _, err := db.StorageDelete(ctx, id, "watcher:heartbeat"); err != nil {
		logging.Debugf("stop watcher:heartbeat cleanup: %v", err)
	}

	clearData, _ := json.Marshal(map[string]interface{}{"phase": nil, "mode": nil})
	if _, err := db.StatePut(ctx, id, clearData, true); err != nil {
		logging.Debugf("stop clear phase failed: %v", err)
	}
	return Allow()
}

func defaultTitle(cwd string) string {
	if cwd == "" {
		return "Claude Code"
	}
	base := filepath.Base(cwd)
	if base == "" || base == "." || base == "/" {
		return "Claude Code"
	}
	return "Claude Code · " + base
}

func buildBody(msg string) string {
	s := strings.TrimSpace(strings.ReplaceAll(msg, "\n", " "))
	s = strings.Join(strings.Fields(s), " ")
	if s == "" {
		return "Session finished"
	}
	r := []rune(s)
	if len(r) > maxBodyLen {
		return string(r[:maxBodyLen-1]) + "…"
	}
	return s
}
