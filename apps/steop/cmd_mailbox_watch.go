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
	since := int64(0)

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
			v, err := strconv.ParseInt(a[len("--since="):], 10, 64)
			if err != nil {
				fmt.Fprintf(os.Stderr, "mailbox watch: invalid --since: %q\n", a[len("--since="):])
				os.Exit(2)
			}
			since = v
		case a == "--json":
			// accepted but ignored — output is always NDJSON
		}
	}

	c, id := mailboxClientAndID()

	seen := make(map[int64]bool)

	// Pre-seed seen set from --since: mark all messages with ID <= since as seen.
	if since > 0 {
		msgs, err := c.MailboxList(id, client.MailboxListOptions{
			Status:      []string{"NEW", "READ", "ARCHIVE"},
			MessageType: msgType,
			Limit:       0,
		})
		if err == nil {
			for _, m := range msgs {
				if m.MessageID <= since {
					seen[m.MessageID] = true
				}
			}
		}
	}

	sigCh := make(chan os.Signal, 1)
	signal.Notify(sigCh, syscall.SIGINT, syscall.SIGTERM)

	// Best-effort watcher lifecycle keys.
	now := time.Now().UTC().Format(time.RFC3339)
	watchState := fmt.Sprintf(`{"status":"watching","task":null,"updated_at":%q}`, now)
	c.StoragePut(id, "watcher:state", watchState)
	c.StoragePut(id, "watcher:heartbeat", now)

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
			c.StoragePut(id, "watcher:heartbeat", time.Now().UTC().Format(time.RFC3339))
		case <-sigCh:
			c.StorageDelete(id, "watcher:state")
			c.StorageDelete(id, "watcher:heartbeat")
			return
		}
	}
}
