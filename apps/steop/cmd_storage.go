package main

import (
	"context"
	"fmt"
	"os"
	"strings"

	"github.com/tasanakorn/stele/apps/steop/internal/client"
	"github.com/tasanakorn/stele/apps/steop/internal/store"
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
	if sessionID == "" && globalSessionID != "" {
		sessionID = globalSessionID
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
	if globalProjectDir != "" {
		c = c.WithRequestContext("", globalProjectDir)
	}
	db, err := openStoreDB()
	if err != nil {
		fmt.Fprintf(os.Stderr, "storage: open db: %v\n", err)
		os.Exit(1)
	}
	defer db.Close()
	ctx := context.Background()

	id := store.Identity{Host: c.Host(), ProjectDir: c.ProjectDir()}
	if sessionID != "" {
		id.SessionID = sessionID
	}

	switch positional[0] {
	case "put":
		if len(positional) < 3 {
			fmt.Fprintln(os.Stderr, "usage: steop storage [--session=<id>] put <key> <content>")
			os.Exit(2)
		}
		meta, err := db.StoragePut(ctx, id, positional[1], positional[2])
		if err != nil {
			fmt.Fprintf(os.Stderr, "storage put: %v\n", err)
			os.Exit(1)
		}
		writeJSON(map[string]interface{}{
			"key":        meta.Key,
			"updated_at": meta.UpdatedAt,
		})
	case "get":
		if len(positional) < 2 {
			fmt.Fprintln(os.Stderr, "usage: steop storage [--session=<id>] get <key>")
			os.Exit(2)
		}
		blob, err := db.StorageGet(ctx, id, positional[1])
		if err != nil {
			fmt.Fprintf(os.Stderr, "storage get: %v\n", err)
			os.Exit(1)
		}
		if blob == nil {
			fmt.Fprintln(os.Stderr, "storage get: not found")
			os.Exit(1)
		}
		writeJSON(blobAsJSON(c, blob))
	case "delete":
		if len(positional) < 2 {
			fmt.Fprintln(os.Stderr, "usage: steop storage [--session=<id>] delete <key>")
			os.Exit(2)
		}
		deleted, err := db.StorageDelete(ctx, id, positional[1])
		if err != nil {
			fmt.Fprintf(os.Stderr, "storage delete: %v\n", err)
			os.Exit(1)
		}
		writeJSON(map[string]interface{}{"deleted": deleted})
	case "list":
		items, err := db.StorageList(ctx, id)
		if err != nil {
			fmt.Fprintf(os.Stderr, "storage list: %v\n", err)
			os.Exit(1)
		}
		out := make([]map[string]interface{}, 0, len(items))
		for _, it := range items {
			out = append(out, map[string]interface{}{
				"key":        it.Key,
				"updated_at": it.UpdatedAt,
				"size":       it.Size,
			})
		}
		writeJSON(map[string]interface{}{"items": out})
	default:
		fmt.Fprintf(os.Stderr, "unknown storage subcommand: %s\n", positional[0])
		os.Exit(2)
	}
}

// blobAsJSON shapes a store.Blob for CLI / watcher output.
func blobAsJSON(c *client.Client, blob *store.Blob) interface{} {
	if blob == nil {
		return nil
	}
	var id string
	if blob.SessionID != "" {
		id = client.ComposeSessionID(blob.Host, blob.ProjectDir, blob.SessionID)
	} else {
		id = client.ComposeProjectID(blob.Host, blob.ProjectDir)
	}
	return map[string]interface{}{
		"id":         id,
		"key":        blob.Key,
		"content":    blob.Content,
		"created_at": blob.CreatedAt,
		"updated_at": blob.UpdatedAt,
	}
}
