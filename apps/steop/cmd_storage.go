package main

import (
	"fmt"
	"os"
	"strings"

	"github.com/tasanakorn/stele/apps/steop/internal/client"
)

func runStorage(args []string) {
	sessionID := ""
	var positional []string
	for _, a := range args {
		if strings.HasPrefix(a, "--session=") {
			sessionID = a[len("--session="):]
		} else {
			positional = append(positional, a)
		}
	}

	if len(positional) < 1 {
		fmt.Fprintln(os.Stderr, "usage: steop storage [--session=<id>] <put|get|delete|list> ...")
		os.Exit(2)
	}
	c, err := client.NewFromConfig()
	if err != nil {
		fmt.Fprintf(os.Stderr, "storage: client init: %v\n", err)
		os.Exit(1)
	}

	// Resolve composite id: session-scoped (3-segment) when --session= is set,
	// otherwise project-scoped (2-segment).
	var id string
	if sessionID != "" {
		id = c.SessionCompositeID(sessionID)
	} else {
		id = c.ProjectID()
	}

	switch positional[0] {
	case "put":
		if len(positional) < 3 {
			fmt.Fprintln(os.Stderr, "usage: steop storage [--session=<id>] put <key> <content>")
			os.Exit(2)
		}
		meta, err := c.StoragePut(id, positional[1], positional[2])
		if err != nil {
			fmt.Fprintf(os.Stderr, "storage put: %v\n", err)
			os.Exit(1)
		}
		writeJSON(meta)
	case "get":
		if len(positional) < 2 {
			fmt.Fprintln(os.Stderr, "usage: steop storage [--session=<id>] get <key>")
			os.Exit(2)
		}
		blob, err := c.StorageGet(id, positional[1])
		if err != nil {
			fmt.Fprintf(os.Stderr, "storage get: %v\n", err)
			os.Exit(1)
		}
		writeJSON(blob)
	case "delete":
		if len(positional) < 2 {
			fmt.Fprintln(os.Stderr, "usage: steop storage [--session=<id>] delete <key>")
			os.Exit(2)
		}
		deleted, err := c.StorageDelete(id, positional[1])
		if err != nil {
			fmt.Fprintf(os.Stderr, "storage delete: %v\n", err)
			os.Exit(1)
		}
		writeJSON(map[string]interface{}{"deleted": deleted})
	case "list":
		items, err := c.StorageList(id)
		if err != nil {
			fmt.Fprintf(os.Stderr, "storage list: %v\n", err)
			os.Exit(1)
		}
		writeJSON(map[string]interface{}{"items": items})
	default:
		fmt.Fprintf(os.Stderr, "unknown storage subcommand: %s\n", positional[0])
		os.Exit(2)
	}
}
