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

	seen := make(map[int64]bool)

	// Fire-and-forget lifecycle writes via FastClone() (500ms timeout).
	now := time.Now().UTC().Format(time.RFC3339)
	watchState := fmt.Sprintf(`{"status":"watching","task":null,"updated_at":%q}`, now)
	fc := c.FastClone()
	go fc.StoragePut(id, "watcher:state", watchState)
	go fc.StoragePut(id, "watcher:heartbeat", now)

	// Emit ready line (PRD-010: immediate feedback for Monitor).
	// PRD-011: no persistent cursor — field is null.
	readyJSON := fmt.Sprintf(`{"type":"ready","last_message_id":null,"interval":%d}`, interval)
	os.Stdout.Write([]byte(readyJSON))
	os.Stdout.Write([]byte("\n"))

	sigCh := make(chan os.Signal, 1)
	signal.Notify(sigCh, syscall.SIGINT, syscall.SIGTERM)

	poll := func() {
		msgs, err := c.MailboxList(id, client.MailboxListOptions{
			Status:      []string{"NEW"},
			MessageType: msgType,
			Limit:       50,
		})
		if err != nil {
			return
		}
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
