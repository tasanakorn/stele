package main

import (
	"context"

	"github.com/tasanakorn/stele/apps/steop/internal/client"
	"github.com/tasanakorn/stele/apps/steop/internal/store"
)

// resolveProjectDir looks up the project_dir from the local store using
// SessionList when CLAUDE_PROJECT_DIR was not available. Mirrors the former
// client.Client.ResolveProjectDir (§4.8 of PRD-020) but reads from the store
// instead of the stele-server.
func resolveProjectDir(db *store.DB, c *client.Client, sessionID string) bool {
	if c == nil {
		return false
	}
	if c.ProjectDir() != "" {
		return true
	}
	if db == nil {
		return false
	}
	sessions, err := db.SessionList(context.Background(), c.Host(), "", "", 0)
	if err != nil {
		return false
	}
	for _, s := range sessions {
		if s.SessionID == sessionID {
			c.SetResolvedProjectDir(s.ProjectDir)
			return true
		}
	}
	return false
}
