package main

import (
	"context"
	"fmt"
	"os"

	"github.com/tasanakorn/stele/apps/steop/internal/client"
)

// runSession dispatches `steop session <start|stop|touch|get|list>`. All
// subcommands route through the local store.
func runSession(args []string) {
	if len(args) < 1 {
		fmt.Fprintln(os.Stderr, "usage: steop session <start|stop|touch|get|list> ...")
		os.Exit(2)
	}
	c, err := client.NewFromConfig()
	if err != nil {
		fmt.Fprintf(os.Stderr, "session: client init: %v\n", err)
		os.Exit(1)
	}
	if globalProjectDir != "" {
		c = c.WithRequestContext("", globalProjectDir)
	}
	db, err := openStoreDB()
	if err != nil {
		fmt.Fprintf(os.Stderr, "session: open db: %v\n", err)
		os.Exit(1)
	}
	defer db.Close()
	ctx := context.Background()

	switch args[0] {
	case "start":
		if len(args) < 2 {
			fmt.Fprintln(os.Stderr, "usage: steop session start <session>")
			os.Exit(2)
		}
		id, err := identFor(c, args[1])
		if err != nil {
			fmt.Fprintf(os.Stderr, "session start: %v\n", err)
			os.Exit(2)
		}
		s, err := db.SessionStart(ctx, id, nil)
		if err != nil {
			fmt.Fprintf(os.Stderr, "session start: %v\n", err)
			os.Exit(1)
		}
		writeJSON(sessionAsJSON(c, s))
	case "stop":
		if len(args) < 2 {
			fmt.Fprintln(os.Stderr, "usage: steop session stop <session>")
			os.Exit(2)
		}
		id, err := identFor(c, args[1])
		if err != nil {
			fmt.Fprintf(os.Stderr, "session stop: %v\n", err)
			os.Exit(2)
		}
		s, err := db.SessionStop(ctx, id)
		if err != nil {
			fmt.Fprintf(os.Stderr, "session stop: %v\n", err)
			os.Exit(1)
		}
		writeJSON(sessionAsJSON(c, s))
	case "touch":
		if len(args) < 2 {
			fmt.Fprintln(os.Stderr, "usage: steop session touch <session>")
			os.Exit(2)
		}
		id, err := identFor(c, args[1])
		if err != nil {
			fmt.Fprintf(os.Stderr, "session touch: %v\n", err)
			os.Exit(2)
		}
		s, err := db.SessionTouch(ctx, id)
		if err != nil {
			fmt.Fprintf(os.Stderr, "session touch: %v\n", err)
			os.Exit(1)
		}
		writeJSON(sessionAsJSON(c, s))
	case "get":
		if len(args) < 2 {
			fmt.Fprintln(os.Stderr, "usage: steop session get <session>")
			os.Exit(2)
		}
		id, err := identFor(c, args[1])
		if err != nil {
			fmt.Fprintf(os.Stderr, "session get: %v\n", err)
			os.Exit(2)
		}
		s, err := db.SessionGet(ctx, id)
		if err != nil {
			fmt.Fprintf(os.Stderr, "session get: %v\n", err)
			os.Exit(1)
		}
		if s == nil {
			fmt.Fprintln(os.Stderr, "session get: not found")
			os.Exit(1)
		}
		writeJSON(sessionAsJSON(c, s))
	case "list":
		sessions, err := db.SessionList(ctx, "", "", "", 0)
		if err != nil {
			fmt.Fprintf(os.Stderr, "session list: %v\n", err)
			os.Exit(1)
		}
		out := make([]interface{}, 0, len(sessions))
		for i := range sessions {
			out = append(out, sessionAsJSON(c, &sessions[i]))
		}
		writeJSON(map[string]interface{}{"sessions": out})
	default:
		fmt.Fprintf(os.Stderr, "unknown session subcommand: %s\n", args[0])
		os.Exit(2)
	}
}
