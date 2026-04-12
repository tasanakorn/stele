package main

import (
	"encoding/json"
	"fmt"
	"os"
	"strings"

	"github.com/tasanakorn/stele/apps/steop/internal/client"
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

	// Resolve target.
	to, err := client.ResolveTarget(c, target)
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
