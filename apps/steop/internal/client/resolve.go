package client

import (
	"fmt"
	"strings"
	"time"
)

// ResolveTarget resolves a short project name to a full composite ID.
// If target already contains ":", it is returned as-is (full ID passthrough).
// Otherwise, active sessions on the local host are searched by project_dir suffix.
func ResolveTarget(c *Client, target string) (string, error) {
	if strings.Contains(target, ":") {
		return target, nil
	}

	sessions, err := c.SessionList(c.Host(), "", "active", 0)
	if err != nil {
		return "", fmt.Errorf("failed to list sessions: %w", err)
	}

	// Group matched sessions by project_dir.
	type match struct {
		id           string
		projectDir   string
		lastActiveAt time.Time
	}
	var matches []match
	seen := make(map[string]bool)

	for _, s := range sessions {
		parts := strings.SplitN(s.ID, ":", 3)
		if len(parts) < 2 {
			continue
		}
		projectDir := parts[1]
		if projectDir != target && !strings.HasSuffix(projectDir, "/"+target) {
			continue
		}
		seen[projectDir] = true
		// Only consider sessions with a real UUID 3rd segment.
		if len(parts) < 3 || parts[2] == "USER" {
			continue
		}
		t, _ := time.Parse(time.RFC3339, s.LastActiveAt)
		matches = append(matches, match{id: s.ID, projectDir: projectDir, lastActiveAt: t})
	}

	if len(matches) == 0 && len(seen) == 0 {
		return "", fmt.Errorf("no active sessions found matching %q", target)
	}

	// No UUID sessions but projectDir was seen — fall back to 2-segment ID.
	if len(matches) == 0 {
		if len(seen) > 1 {
			var dirs []string
			for d := range seen {
				dirs = append(dirs, "  "+d)
			}
			return "", fmt.Errorf("ambiguous target %q matches multiple projects:\n%s\nUse a more specific name or pass a full composite ID.", target, strings.Join(dirs, "\n"))
		}
		for d := range seen {
			return c.Host() + ":" + d, nil
		}
	}

	if len(seen) > 1 {
		var dirs []string
		for d := range seen {
			dirs = append(dirs, "  "+d)
		}
		return "", fmt.Errorf("ambiguous target %q matches multiple projects:\n%s\nUse a more specific name or pass a full composite ID.", target, strings.Join(dirs, "\n"))
	}

	// Single project_dir — pick the session with the latest last_active_at.
	best := matches[0]
	for _, m := range matches[1:] {
		if m.lastActiveAt.After(best.lastActiveAt) {
			best = m
		}
	}

	return best.id, nil
}
