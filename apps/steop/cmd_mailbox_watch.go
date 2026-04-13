package main

import (
	"encoding/json"
	"fmt"
	"os"
	"os/signal"
	"strconv"
	"strings"
	"syscall"
	"time"

	"github.com/tasanakorn/stele/apps/steop/internal/client"
)

func runMailboxWatch(args []string) {
	msgType := ""
	interval := 10

	for _, a := range args {
		switch {
		case strings.HasPrefix(a, "--type="):
			msgType = a[len("--type="):]
		case strings.HasPrefix(a, "--interval="):
			v, err := strconv.Atoi(a[len("--interval="):])
			if err != nil || v < 2 {
				fmt.Fprintf(os.Stderr, "mailbox watch: --interval must be >= 2\n")
				os.Exit(2)
			}
			if v > 300 {
				v = 300
			}
			interval = v
		case strings.HasPrefix(a, "--since="):
			fmt.Fprintf(os.Stderr, "mailbox watch: --since is deprecated; resume is automatic via server-side status=NEW filter\n")
		case a == "--json":
			// accepted but ignored — output is always NDJSON
		}
	}

	c, id := mailboxClientAndID()

	// Build the set of mailbox IDs to poll. The watcher must cover both the
	// project-level (2-segment) and session-level (3-segment) mailboxes so that
	// senders can address either form and the message still arrives.
	//
	// If --x-session-id was supplied, id is already 3-segment; add the 2-segment
	// project-level ID as well. Otherwise id is 2-segment; look up the most
	// recently active UUID session for this project and add that 3-segment ID.
	pollIDs := []string{id}
	if globalSessionID != "" {
		// id is session-level — also poll project-level
		pollIDs = append(pollIDs, c.ProjectID())
	} else {
		// id is project-level — also poll the most recently active session
		if sid := latestSessionID(c); sid != "" {
			pollIDs = append(pollIDs, sid)
		}
	}

	// Fire-and-forget lifecycle writes via FastClone() (500ms timeout).
	now := time.Now().UTC().Format(time.RFC3339)
	watchState := fmt.Sprintf(`{"status":"watching","task":null,"updated_at":%q}`, now)
	fc := c.FastClone()
	go fc.StoragePut(id, "watcher:state", watchState)
	go fc.StoragePut(id, "watcher:heartbeat", now)

	sigCh := make(chan os.Signal, 1)
	signal.Notify(sigCh, syscall.SIGINT, syscall.SIGTERM)

	// seen tracks message IDs already emitted in this poll cycle to avoid
	// duplicates when the same message appears in multiple mailboxes.
	seen := make(map[int64]bool)

	poll := func() {
		for k := range seen {
			delete(seen, k)
		}
		for _, pid := range pollIDs {
			msgs, err := c.MailboxList(pid, client.MailboxListOptions{
				Status:      []string{"NEW"},
				MessageType: msgType,
				Limit:       50,
			})
			if err != nil {
				continue
			}
			// At-least-once delivery: re-emit every NEW message on every poll
			// until the consumer claims it (status=NEW → CLAIMED via mailbox read).
			// Dedup at the consumer is via HTTP 409 on duplicate claim (PRD-009).
			for _, m := range msgs {
				if seen[m.MessageID] {
					continue
				}
				seen[m.MessageID] = true
				b, err := json.Marshal(m)
				if err != nil {
					continue
				}
				os.Stdout.Write(b)
				os.Stdout.Write([]byte("\n"))
			}
		}
	}

	// Emit a ready handshake so consumers know the watcher is alive and
	// have received the effective poll interval. Must be the first line
	// on stdout, before the initial poll() so any matching NEW messages
	// appear after READY.
	readyLine, err := json.Marshal(struct {
		MessageType string `json:"message_type"`
		Interval    int    `json:"interval"`
	}{
		MessageType: "WATCHER:READY",
		Interval:    interval,
	})
	if err == nil {
		os.Stdout.Write(readyLine)
		os.Stdout.Write([]byte("\n"))
	}

	// Immediate first poll.
	poll()

	ticker := time.NewTicker(time.Duration(interval) * time.Second)
	defer ticker.Stop()

	for {
		select {
		case <-ticker.C:
			poll()
			now := time.Now().UTC().Format(time.RFC3339)
			c.StoragePut(id, "watcher:state", fmt.Sprintf(`{"status":"watching","task":null,"updated_at":%q}`, now))
			c.StoragePut(id, "watcher:heartbeat", now)
		case <-sigCh:
			c.StorageDelete(id, "watcher:state")
			c.StorageDelete(id, "watcher:heartbeat")
			return
		}
	}
}

// latestSessionID returns the 3-segment composite ID of the most recently active
// UUID session for this client's project, or "" if none can be found. Used by
// the watcher to also poll the session-level mailbox when started without an
// explicit --x-session-id.
func latestSessionID(c *client.Client) string {
	// Guard: without a project_dir filter the server returns sessions for all
	// projects on the host, which would pick up an unrelated session.
	if c.ProjectDir() == "" {
		return ""
	}
	sessions, err := c.SessionList(c.Host(), c.ProjectDir(), "active", 0)
	if err != nil {
		return ""
	}
	var best *client.Session
	var bestTime time.Time
	for i, s := range sessions {
		parts := strings.SplitN(s.ID, ":", 3)
		if len(parts) != 3 || parts[2] == "USER" {
			continue
		}
		t, _ := time.Parse(time.RFC3339Nano, s.LastActiveAt)
		if best == nil || t.After(bestTime) {
			best = &sessions[i]
			bestTime = t
		}
	}
	if best == nil {
		return ""
	}
	return best.ID
}
