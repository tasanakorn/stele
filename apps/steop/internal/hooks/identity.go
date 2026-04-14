package hooks

import (
	"github.com/tasanakorn/stele/apps/steop/internal/client"
	"github.com/tasanakorn/stele/apps/steop/internal/store"
)

// sessionIdent builds a 3-segment store.Identity from the client's host /
// project_dir and the hook's session_id. Returns ok=false when any segment
// is empty — caller should skip the store write in that case.
func sessionIdent(c *client.Client, sessionID string) (store.Identity, bool) {
	if c == nil || sessionID == "" {
		return store.Identity{}, false
	}
	if c.Host() == "" || c.ProjectDir() == "" {
		return store.Identity{}, false
	}
	return store.Identity{
		Host:       c.Host(),
		ProjectDir: c.ProjectDir(),
		SessionID:  sessionID,
	}, true
}

// projectIdent builds a 2-segment store.Identity from the client's host /
// project_dir.
func projectIdent(c *client.Client) (store.Identity, bool) {
	if c == nil || c.Host() == "" || c.ProjectDir() == "" {
		return store.Identity{}, false
	}
	return store.Identity{
		Host:       c.Host(),
		ProjectDir: c.ProjectDir(),
	}, true
}
