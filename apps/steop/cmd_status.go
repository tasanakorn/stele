package main

import (
	"context"
	"fmt"
	"os"

	"github.com/tasanakorn/stele/apps/steop/internal/client"
)

// runStatus dispatches `steop status <session>` — returns the statusline
// projection via store.StatusGet.
func runStatus(args []string) {
	if len(args) < 1 {
		fmt.Fprintln(os.Stderr, "usage: steop status <session>")
		os.Exit(2)
	}
	c, err := client.NewFromConfig()
	if err != nil {
		fmt.Fprintf(os.Stderr, "status: client init: %v\n", err)
		os.Exit(1)
	}
	if globalProjectDir != "" {
		c = c.WithRequestContext("", globalProjectDir)
	}
	db, err := openStoreDB()
	if err != nil {
		fmt.Fprintf(os.Stderr, "status: open db: %v\n", err)
		os.Exit(1)
	}
	defer db.Close()

	id, err := identFor(c, args[0])
	if err != nil {
		fmt.Fprintf(os.Stderr, "status: %v\n", err)
		os.Exit(2)
	}
	s, err := db.StatusGet(context.Background(), id)
	if err != nil {
		fmt.Fprintf(os.Stderr, "status: %v\n", err)
		os.Exit(1)
	}
	writeJSON(statusAsJSON(c, s))
}
