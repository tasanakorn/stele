package main

import (
	"context"
	"encoding/json"
	"fmt"
	"os"
	"os/signal"
	"strconv"
	"strings"
	"syscall"
	"time"

	"github.com/tasanakorn/stele/apps/steop/internal/client"
	"github.com/tasanakorn/stele/apps/steop/internal/logging"
	"github.com/tasanakorn/stele/apps/steop/internal/store"
)

const throttleTimeout = 5 * time.Minute

// metaTaskStatus reads the reserved meta.task_status string key from a
// MailboxMessage. Returns "" for any shape that is not a JSON object with a
// string task_status field. Used by the watcher's throttle gate.
func metaTaskStatus(m client.MailboxMessage) string {
	obj, ok := m.Meta.(map[string]interface{})
	if !ok {
		return ""
	}
	s, _ := obj["task_status"].(string)
	return s
}

func runMailboxWatch(args []string) {
	msgType := ""
	interval := 10

	for i := 0; i < len(args); i++ {
		a := args[i]
		// flagVal returns the value for a flag, supporting both --flag=value and
		// --flag value forms. Returns ("", false) if a doesn't match the prefix.
		flagVal := func(prefix string) (string, bool) {
			eq := prefix + "="
			if strings.HasPrefix(a, eq) {
				return a[len(eq):], true
			}
			if a == prefix && i+1 < len(args) {
				i++
				return args[i], true
			}
			return "", false
		}
		switch {
		case strings.HasPrefix(a, "--type"):
			if v, ok := flagVal("--type"); ok {
				msgType = v
			}
		case strings.HasPrefix(a, "--interval"):
			if v, ok := flagVal("--interval"); ok {
				n, err := strconv.Atoi(v)
				if err != nil || n < 2 {
					fmt.Fprintf(os.Stderr, "mailbox watch: --interval must be >= 2\n")
					os.Exit(2)
				}
				if n > 300 {
					n = 300
				}
				interval = n
			}
		case strings.HasPrefix(a, "--since"):
			fmt.Fprintf(os.Stderr, "mailbox watch: --since is deprecated; resume is automatic via server-side status=NEW filter\n")
		case a == "--json":
			// accepted but ignored — output is always NDJSON
		}
	}

	c, id := mailboxClientAndID()

	db, err := openStoreDB()
	if err != nil {
		fmt.Fprintf(os.Stderr, "mailbox watch: open db: %v\n", err)
		os.Exit(1)
	}
	defer db.Close()
	ctx := context.Background()

	// Build watcher's own store identity for local KV writes (watcher:state /
	// watcher:heartbeat). Mirrors the mailbox id's arity.
	watcherStoreID := store.Identity{Host: c.Host(), ProjectDir: c.ProjectDir()}
	if globalSessionID != "" {
		watcherStoreID.SessionID = globalSessionID
	}

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
		if sid := latestSessionID(ctx, db, c); sid != "" {
			pollIDs = append(pollIDs, sid)
		}
	}

	now := time.Now().UTC().Format(time.RFC3339)
	watchState := fmt.Sprintf(`{"status":"watching","task":null,"updated_at":%q}`, now)
	if _, err := db.StoragePut(ctx, watcherStoreID, "watcher:state", watchState); err != nil {
		logging.Debugf("watcher: storage put watcher:state: %v", err)
	}
	if _, err := db.StoragePut(ctx, watcherStoreID, "watcher:heartbeat", now); err != nil {
		logging.Debugf("watcher: storage put watcher:heartbeat: %v", err)
	}

	sigCh := make(chan os.Signal, 1)
	signal.Notify(sigCh, syscall.SIGINT, syscall.SIGTERM)

	// seen tracks message IDs already emitted in this poll cycle to avoid
	// duplicates when the same message appears in multiple mailboxes.
	seen := make(map[int64]bool)

	// Throttle state: after emitting one TASK:REQUEST line, suppress further
	// stdout emissions until the held row is acked via meta.task_status=DONE,
	// disappears from the poll batch (naked archive), or the 5-minute deadline
	// elapses. Polling continues during suspend; only stdout is gated.
	var throttleActive bool
	var throttleMsgID int64
	var throttleDeadline time.Time

	poll := func() {
		for k := range seen {
			delete(seen, k)
		}
		// Collect the merged batch across pollIDs first so the throttle gate
		// can inspect the full set (needed to detect the held row's meta
		// transition or disappearance even when pollIDs return it in different
		// orders).
		var batch []client.MailboxMessage
		for _, pid := range pollIDs {
			msgs, err := c.MailboxList(pid, client.MailboxListOptions{
				Status:      []string{"NEW"},
				MessageType: msgType,
				Limit:       50,
			})
			if err != nil {
				continue
			}
			for _, m := range msgs {
				if seen[m.MessageID] {
					continue
				}
				seen[m.MessageID] = true
				batch = append(batch, m)
			}
		}

		// Throttle resume checks: observe the held row in the batch.
		if throttleActive {
			if time.Now().After(throttleDeadline) {
				fmt.Fprintf(os.Stderr, "watcher: throttle timeout for message_id=%d, resuming\n", throttleMsgID)
				throttleActive = false
			} else {
				var held *client.MailboxMessage
				for i := range batch {
					if batch[i].MessageID == throttleMsgID {
						held = &batch[i]
						break
					}
				}
				if held == nil {
					// Row disappeared from NEW — naked archive, treat as
					// implicit completion and resume.
					fmt.Fprintf(os.Stderr, "watcher: held message_id=%d no longer in batch, resuming\n", throttleMsgID)
					throttleActive = false
				} else if metaTaskStatus(*held) == "DONE" {
					fmt.Fprintf(os.Stderr, "watcher: DONE ack received for message_id=%d, resuming\n", throttleMsgID)
					throttleActive = false
				}
			}
		}

		// At-least-once delivery: re-emit every NEW message on every poll
		// until the consumer claims it (status=NEW → CLAIMED via mailbox read).
		// Dedup at the consumer is via HTTP 409 on duplicate claim (PRD-009).
		for _, m := range batch {
			// Gate only TASK:REQUEST stdout emissions behind the throttle.
			if m.MessageType == "TASK:REQUEST" && throttleActive {
				continue
			}
			b, err := json.Marshal(m)
			if err != nil {
				continue
			}
			os.Stdout.Write(append(b, '\n'))
			if m.MessageType == "TASK:REQUEST" {
				throttleActive = true
				throttleMsgID = m.MessageID
				throttleDeadline = time.Now().Add(throttleTimeout)
			}
		}
	}

	// Emit a ready handshake so consumers know the watcher is alive and
	// have received the effective poll interval. Must be the first line
	// on stdout, before the initial poll() so any matching NEW messages
	// appear after READY.
	readyProjectDir := c.ProjectDir()
	readySessionID := globalSessionID
	if readySessionID == "" {
		for _, pid := range pollIDs {
			parts := strings.SplitN(pid, ":", 3)
			if len(parts) == 3 && parts[2] != "USER" {
				readySessionID = parts[2]
				break
			}
		}
	}
	readyLine, err := json.Marshal(struct {
		MessageType string `json:"message_type"`
		Interval    int    `json:"interval"`
		SessionID   string `json:"session_id,omitempty"`
		ProjectDir  string `json:"project_dir,omitempty"`
	}{
		MessageType: "WATCHER:READY",
		Interval:    interval,
		SessionID:   readySessionID,
		ProjectDir:  readyProjectDir,
	})
	if err == nil {
		os.Stdout.Write(append(readyLine, '\n'))
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
			db.StoragePut(ctx, watcherStoreID, "watcher:state", fmt.Sprintf(`{"status":"watching","task":null,"updated_at":%q}`, now))
			db.StoragePut(ctx, watcherStoreID, "watcher:heartbeat", now)
		case <-sigCh:
			db.StorageDelete(ctx, watcherStoreID, "watcher:state")
			db.StorageDelete(ctx, watcherStoreID, "watcher:heartbeat")
			return
		}
	}
}

// latestSessionID returns the 3-segment composite ID of the most recently active
// UUID session for this client's project, or "" if none can be found. Used by
// the watcher to also poll the session-level mailbox when started without an
// explicit --x-session-id.
func latestSessionID(ctx context.Context, db *store.DB, c *client.Client) string {
	// Guard: without a project_dir filter we'd pick up an unrelated session.
	if c.ProjectDir() == "" {
		return ""
	}
	sessions, err := db.SessionList(ctx, c.Host(), c.ProjectDir(), "active", 0)
	if err != nil {
		return ""
	}
	var best *store.Session
	var bestTime int64
	for i, s := range sessions {
		if s.SessionID == "" || s.SessionID == "USER" {
			continue
		}
		if best == nil || s.LastActiveAt > bestTime {
			best = &sessions[i]
			bestTime = s.LastActiveAt
		}
	}
	if best == nil {
		return ""
	}
	return client.ComposeSessionID(best.Host, best.ProjectDir, best.SessionID)
}
