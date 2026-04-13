package main

import (
	"encoding/json"
	"fmt"
	"os"
	"strconv"
	"strings"

	"github.com/tasanakorn/stele/apps/steop/internal/client"
)

func runMailbox(args []string) {
	if len(args) < 1 {
		fmt.Fprintln(os.Stderr, "usage: steop mailbox <list|get|send|read|archive|update-meta|watch> ...")
		os.Exit(2)
	}
	switch args[0] {
	case "list":
		runMailboxList(args[1:])
	case "get":
		runMailboxGet(args[1:])
	case "send":
		runMailboxSend(args[1:])
	case "read":
		runMailboxRead(args[1:])
	case "archive":
		runMailboxArchive(args[1:])
	case "update-meta":
		runMailboxUpdateMeta(args[1:])
	case "watch":
		runMailboxWatch(args[1:])
	default:
		fmt.Fprintf(os.Stderr, "unknown mailbox subcommand: %s\n", args[0])
		os.Exit(2)
	}
}

// mailboxClientAndID creates a client and resolves the composite identity.
// If globalSessionID is set, returns a 3-segment session-scoped ID;
// otherwise returns a 2-segment project-scoped ID.
func mailboxClientAndID() (*client.Client, string) {
	c, err := client.NewFromConfig()
	if err != nil {
		fmt.Fprintf(os.Stderr, "mailbox: client init: %v\n", err)
		os.Exit(1)
	}
	if globalProjectDir != "" {
		c = c.WithRequestContext("", globalProjectDir)
	}
	var id string
	if globalSessionID != "" {
		id = c.SessionCompositeID(globalSessionID)
	} else {
		id = c.ProjectID()
	}
	return c, id
}

func runMailboxList(args []string) {
	msgType := ""
	status := "NEW"
	limit := 20
	jsonOut := false

	var positional []string
	for _, a := range args {
		switch {
		case strings.HasPrefix(a, "--type="):
			msgType = a[len("--type="):]
		case strings.HasPrefix(a, "--status="):
			status = a[len("--status="):]
		case strings.HasPrefix(a, "--limit="):
			v, err := strconv.Atoi(a[len("--limit="):])
			if err != nil || v <= 0 {
				fmt.Fprintf(os.Stderr, "mailbox list: invalid --limit: %q\n", a[len("--limit="):])
				os.Exit(2)
			}
			limit = v
		case a == "--json":
			jsonOut = true
		default:
			positional = append(positional, a)
		}
	}
	_ = positional

	c, id := mailboxClientAndID()

	var statuses []string
	if strings.EqualFold(status, "ALL") {
		statuses = []string{"NEW", "READ", "ARCHIVE"}
	} else {
		statuses = strings.Split(strings.ToUpper(status), ",")
	}

	msgs, err := c.MailboxList(id, client.MailboxListOptions{
		Status:      statuses,
		MessageType: msgType,
		Limit:       limit,
	})
	if err != nil {
		fmt.Fprintf(os.Stderr, "mailbox list: %v\n", err)
		os.Exit(1)
	}

	if jsonOut {
		writeJSON(msgs)
		return
	}

	if len(msgs) == 0 {
		fmt.Println("(no messages)")
		return
	}
	fmt.Printf("%-6s  %-34s  %-15s  %-24s  %-7s  %s\n", "ID", "FROM", "TYPE", "SUBJECT", "STATUS", "CREATED_AT")
	for _, m := range msgs {
		subj := m.Subject
		if len(subj) > 24 {
			subj = subj[:23] + "…"
		}
		fmt.Printf("%-6d  %-34s  %-15s  %-24s  %-7s  %s\n",
			m.MessageID, truncate(m.From, 34), truncate(m.MessageType, 15), subj, m.Status, m.CreatedAt)
	}
}

func runMailboxGet(args []string) {
	jsonOut := false
	var positional []string
	for _, a := range args {
		if a == "--json" {
			jsonOut = true
		} else {
			positional = append(positional, a)
		}
	}
	if len(positional) < 1 {
		fmt.Fprintln(os.Stderr, "usage: steop mailbox get <message_id> [--json]")
		os.Exit(2)
	}
	messageID, err := strconv.ParseInt(positional[0], 10, 64)
	if err != nil {
		fmt.Fprintf(os.Stderr, "mailbox get: invalid message_id: %q\n", positional[0])
		os.Exit(2)
	}

	c, id := mailboxClientAndID()
	msg, err := c.MailboxGet(id, messageID)
	if err != nil {
		fmt.Fprintf(os.Stderr, "mailbox get: %v\n", err)
		os.Exit(1)
	}

	if jsonOut {
		writeJSON(msg)
		return
	}
	printMailboxMessage(msg)
}

func runMailboxSend(args []string) {
	to := ""
	msgType := ""
	subject := ""
	var metaRaw, payloadRaw string

	for _, a := range args {
		switch {
		case strings.HasPrefix(a, "--to="):
			to = a[len("--to="):]
		case strings.HasPrefix(a, "--type="):
			msgType = a[len("--type="):]
		case strings.HasPrefix(a, "--subject="):
			subject = a[len("--subject="):]
		case strings.HasPrefix(a, "--meta="):
			metaRaw = a[len("--meta="):]
		case strings.HasPrefix(a, "--payload="):
			payloadRaw = a[len("--payload="):]
		}
	}

	if to == "" {
		fmt.Fprintln(os.Stderr, "usage: steop mailbox send --to=<to> [--type=TYPE] [--subject=SUBJECT] [--meta=JSON] [--payload=JSON]")
		os.Exit(2)
	}

	c, id := mailboxClientAndID()

	opts := client.MailboxSendOptions{
		MessageType: msgType,
		Subject:     subject,
	}
	if metaRaw != "" {
		var meta interface{}
		if err := json.Unmarshal([]byte(metaRaw), &meta); err != nil {
			fmt.Fprintf(os.Stderr, "mailbox send: invalid --meta JSON: %v\n", err)
			os.Exit(2)
		}
		opts.Meta = meta
	}
	if payloadRaw != "" {
		var payload interface{}
		if err := json.Unmarshal([]byte(payloadRaw), &payload); err != nil {
			fmt.Fprintf(os.Stderr, "mailbox send: invalid --payload JSON: %v\n", err)
			os.Exit(2)
		}
		opts.Payload = payload
	}

	msg, err := c.MailboxSend(id, to, opts)
	if err != nil {
		fmt.Fprintf(os.Stderr, "mailbox send: %v\n", err)
		os.Exit(1)
	}
	writeJSON(msg)
}

func runMailboxRead(args []string) {
	if len(args) < 1 {
		fmt.Fprintln(os.Stderr, "usage: steop mailbox read <message_id>")
		os.Exit(2)
	}
	messageID, err := strconv.ParseInt(args[0], 10, 64)
	if err != nil {
		fmt.Fprintf(os.Stderr, "mailbox read: invalid message_id: %q\n", args[0])
		os.Exit(2)
	}

	c, id := mailboxClientAndID()
	if err := c.MailboxRead(id, messageID); err != nil {
		fmt.Fprintf(os.Stderr, "mailbox read: %v\n", err)
		os.Exit(1)
	}
	writeJSON(map[string]interface{}{"message_id": messageID, "status": "READ"})
}

func runMailboxUpdateMeta(args []string) {
	if len(args) < 2 {
		fmt.Fprintln(os.Stderr, "usage: steop mailbox update-meta <message_id> <meta-json>")
		os.Exit(2)
	}
	messageID, err := strconv.ParseInt(args[0], 10, 64)
	if err != nil {
		fmt.Fprintf(os.Stderr, "mailbox update-meta: invalid message_id: %q\n", args[0])
		os.Exit(2)
	}
	var metaPatch interface{}
	if err := json.Unmarshal([]byte(args[1]), &metaPatch); err != nil {
		fmt.Fprintf(os.Stderr, "mailbox update-meta: invalid meta JSON: %v\n", err)
		os.Exit(2)
	}

	c, id := mailboxClientAndID()
	msg, err := c.MailboxUpdateMeta(id, messageID, metaPatch)
	if err != nil {
		fmt.Fprintf(os.Stderr, "mailbox update-meta: %v\n", err)
		os.Exit(1)
	}
	writeJSON(msg)
}

func runMailboxArchive(args []string) {
	if len(args) < 1 {
		fmt.Fprintln(os.Stderr, "usage: steop mailbox archive <message_id>")
		os.Exit(2)
	}
	messageID, err := strconv.ParseInt(args[0], 10, 64)
	if err != nil {
		fmt.Fprintf(os.Stderr, "mailbox archive: invalid message_id: %q\n", args[0])
		os.Exit(2)
	}

	c, id := mailboxClientAndID()
	if err := c.MailboxArchive(id, messageID); err != nil {
		fmt.Fprintf(os.Stderr, "mailbox archive: %v\n", err)
		os.Exit(1)
	}
	writeJSON(map[string]interface{}{"message_id": messageID, "status": "ARCHIVE"})
}

func printMailboxMessage(m *client.MailboxMessage) {
	fmt.Printf("message_id  : %d\n", m.MessageID)
	fmt.Printf("from        : %s\n", m.From)
	fmt.Printf("to          : %s\n", m.To)
	fmt.Printf("subject     : %s\n", m.Subject)
	fmt.Printf("message_type: %s\n", m.MessageType)
	fmt.Printf("status      : %s\n", m.Status)
	fmt.Printf("created_at  : %s\n", m.CreatedAt)
	fmt.Println("meta:")
	printIndentedJSON(m.Meta)
	fmt.Println("payload:")
	printIndentedJSON(m.Payload)
}

func printIndentedJSON(v interface{}) {
	if v == nil {
		fmt.Println("  (empty)")
		return
	}
	b, err := json.MarshalIndent(v, "  ", "  ")
	if err != nil {
		fmt.Printf("  %v\n", v)
		return
	}
	fmt.Printf("  %s\n", string(b))
}

func truncate(s string, max int) string {
	r := []rune(s)
	if len(r) <= max {
		return s
	}
	return string(r[:max-1]) + "…"
}
