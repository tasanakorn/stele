package hooks

import (
	"path/filepath"
	"strings"
	"time"

	"github.com/tasanakorn/stele/apps/steop/internal/client"
	"github.com/tasanakorn/stele/apps/steop/internal/logging"
)

const maxBodyLen = 140

// HandleStop fires a desktop notification via stele-server, posts a session
// summary to the inbox endpoint, and clears transient state for the session.
// Always returns Allow() to honor the non-blocking contract.
func HandleStop(in *HookInput, c *client.Client) []byte {
	if in == nil || c == nil {
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

	state, stateErr := c.StateGet(in.SessionID)
	if stateErr != nil {
		logging.Debugf("stop state get failed: %v", stateErr)
	}

	if state != nil {
		payload := map[string]interface{}{
			"cwd":      in.Cwd,
			"data":     state.Data,
			"counters": state.Counters,
			"ended_at": time.Now().UTC().Format(time.RFC3339),
		}
		if _, err := c.MailboxSendFromSelf(in.SessionID, c.Host(), c.ProjectDir(), "", payload); err != nil {
			logging.Debugf("stop mailbox send failed: %v", err)
		}
		if pm, ok := state.Data["persistent_mode"].(bool); ok && pm {
			logging.Debugf("persistent_mode set but not honored in v1")
		}
	}

	if _, err := c.StatePut(c.Host(), c.ProjectDir(), in.SessionID, map[string]interface{}{"phase": nil, "mode": nil}, true); err != nil {
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
