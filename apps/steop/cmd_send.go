package main

import (
	"context"
	"encoding/json"
	"fmt"
	"os"
	"strings"

	"github.com/tasanakorn/stele/apps/steop/internal/client"
	"github.com/tasanakorn/stele/apps/steop/internal/store"
)

func runSend(args []string) {
	mode := "normal"
	subject := ""
	metaExtra := ""
	var positional []string

	for _, a := range args {
		switch {
		case strings.HasPrefix(a, "--mode="):
			mode = a[len("--mode="):]
		case strings.HasPrefix(a, "--subject="):
			subject = a[len("--subject="):]
		case strings.HasPrefix(a, "--meta="):
			metaExtra = a[len("--meta="):]
		default:
			positional = append(positional, a)
		}
	}

	if len(positional) < 2 {
		fmt.Fprintln(os.Stderr, "usage: steop send <target> <description> [--mode=normal|flow] [--subject=SUBJECT] [--meta=JSON]")
		os.Exit(2)
	}

	if mode != "normal" && mode != "flow" {
		fmt.Fprintf(os.Stderr, "invalid mode %q, must be 'normal' or 'flow'\n", mode)
		os.Exit(2)
	}

	target := positional[0]
	description := strings.Join(positional[1:], " ")

	if subject == "" {
		r := []rune(description)
		if len(r) > 80 {
			subject = string(r[:80])
		} else {
			subject = description
		}
	}

	// Build meta.
	meta := map[string]interface{}{
		"description": description,
		"mode":        mode,
	}
	if metaExtra != "" {
		var extra map[string]interface{}
		if err := json.Unmarshal([]byte(metaExtra), &extra); err != nil {
			fmt.Fprintf(os.Stderr, "invalid --meta JSON: %v\n", err)
			os.Exit(2)
		}
		for k, v := range extra {
			meta[k] = v
		}
	}

	// Init client and resolve identity.
	c, err := client.NewFromConfig()
	if err != nil {
		fmt.Fprintf(os.Stderr, "send: client init: %v\n", err)
		os.Exit(1)
	}
	if globalProjectDir != "" {
		c = c.WithRequestContext("", globalProjectDir)
	}

	db, err := openStoreDB()
	if err != nil {
		fmt.Fprintf(os.Stderr, "send: open db: %v\n", err)
		os.Exit(1)
	}
	defer db.Close()

	// Resolve target.
	to, err := resolveSendTarget(db, c, target)
	if err != nil {
		fmt.Fprintf(os.Stderr, "send: %v\n", err)
		os.Exit(1)
	}

	// Sender identity.
	var id string
	if globalSessionID != "" {
		id = c.SessionCompositeID(globalSessionID)
	} else {
		id = c.ProjectID()
	}

	// Send.
	msg, err := c.MailboxSend(id, to, client.MailboxSendOptions{
		Subject:     subject,
		MessageType: "TASK:REQUEST",
		Meta:        meta,
	})
	if err != nil {
		fmt.Fprintf(os.Stderr, "send: %v\n", err)
		os.Exit(1)
	}

	fmt.Printf("sent task to %s (message_id: %d, mode: %s)\n", to, msg.MessageID, mode)
}

// resolveSendTarget resolves a short project name to a full composite ID.
// If target already contains ":", it is returned as-is (full ID passthrough).
// Otherwise, active sessions on the local host are searched by project_dir
// suffix in the local store.
func resolveSendTarget(db *store.DB, c *client.Client, target string) (string, error) {
	if strings.Contains(target, ":") {
		return target, nil
	}

	sessions, err := db.SessionList(context.Background(), c.Host(), "", "active", 0)
	if err != nil {
		return "", fmt.Errorf("failed to list sessions: %w", err)
	}

	type match struct {
		id           string
		projectDir   string
		lastActiveAt int64
	}
	var matches []match
	seen := make(map[string]bool)

	for _, s := range sessions {
		projectDir := s.ProjectDir
		if projectDir != target && !strings.HasSuffix(projectDir, "/"+target) {
			continue
		}
		seen[projectDir] = true
		// Only consider sessions with a real UUID 3rd segment.
		if s.SessionID == "" || s.SessionID == "USER" {
			continue
		}
		matches = append(matches, match{
			id:           client.ComposeSessionID(s.Host, s.ProjectDir, s.SessionID),
			projectDir:   projectDir,
			lastActiveAt: s.LastActiveAt,
		})
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
			return client.ComposeProjectID(c.Host(), d), nil
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
		if m.lastActiveAt > best.lastActiveAt {
			best = m
		}
	}

	return best.id, nil
}
