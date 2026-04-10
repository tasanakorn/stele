package main

import (
	"fmt"
	"os"

	"github.com/tasanakorn/stele/apps/steop/internal/client"
)

func runStorage(args []string) {
	if len(args) < 1 {
		fmt.Fprintln(os.Stderr, "usage: steop storage <put|get|delete|list> ...")
		os.Exit(2)
	}
	c, err := client.NewFromConfig()
	if err != nil {
		fmt.Fprintf(os.Stderr, "storage: client init: %v\n", err)
		os.Exit(1)
	}

	switch args[0] {
	case "put":
		if len(args) < 4 {
			fmt.Fprintln(os.Stderr, "usage: steop storage put <scope> <key> <content>")
			os.Exit(2)
		}
		meta, err := c.StoragePut(args[1], args[2], args[3])
		if err != nil {
			fmt.Fprintf(os.Stderr, "storage put: %v\n", err)
			os.Exit(1)
		}
		writeJSON(meta)
	case "get":
		if len(args) < 3 {
			fmt.Fprintln(os.Stderr, "usage: steop storage get <scope> <key>")
			os.Exit(2)
		}
		blob, err := c.StorageGet(args[1], args[2])
		if err != nil {
			fmt.Fprintf(os.Stderr, "storage get: %v\n", err)
			os.Exit(1)
		}
		writeJSON(blob)
	case "delete":
		if len(args) < 3 {
			fmt.Fprintln(os.Stderr, "usage: steop storage delete <scope> <key>")
			os.Exit(2)
		}
		deleted, err := c.StorageDelete(args[1], args[2])
		if err != nil {
			fmt.Fprintf(os.Stderr, "storage delete: %v\n", err)
			os.Exit(1)
		}
		writeJSON(map[string]interface{}{"deleted": deleted})
	case "list":
		if len(args) < 2 {
			fmt.Fprintln(os.Stderr, "usage: steop storage list <scope>")
			os.Exit(2)
		}
		items, err := c.StorageList(args[1])
		if err != nil {
			fmt.Fprintf(os.Stderr, "storage list: %v\n", err)
			os.Exit(1)
		}
		writeJSON(map[string]interface{}{"items": items})
	default:
		fmt.Fprintf(os.Stderr, "unknown storage subcommand: %s\n", args[0])
		os.Exit(2)
	}
}
