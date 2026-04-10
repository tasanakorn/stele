package hooks

import (
	"path/filepath"
	"strings"

	"github.com/tasanakorn/stele/apps/steop/internal/client"
	"github.com/tasanakorn/stele/apps/steop/internal/logging"
)

const maxBodyLen = 140

// HandleStop fires a desktop notification via stele-server.
// Always returns Allow() to honor the non-blocking contract.
func HandleStop(in *HookInput, c *client.Client) []byte {
	req := client.NotifyRequest{
		Title: defaultTitle(in.Cwd),
		Body:  buildBody(in.LastAssistantMessage),
	}
	if err := c.Notify(req); err != nil {
		logging.Debugf("notify call failed: %v", err)
	}
	if in != nil && in.SessionID != "" {
		data := map[string]interface{}{"phase": "", "mode": ""}
		if _, err := c.StatePut(in.SessionID, data, true); err != nil {
			logging.Debugf("stop clear phase failed: %v", err)
		}
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
