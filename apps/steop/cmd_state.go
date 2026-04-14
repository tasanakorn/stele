package main

import (
	"context"
	"encoding/json"
	"fmt"
	"os"
	"strconv"

	"github.com/tasanakorn/stele/apps/steop/internal/client"
	"github.com/tasanakorn/stele/apps/steop/internal/datadir"
	"github.com/tasanakorn/stele/apps/steop/internal/store"
)

func runState(args []string) {
	if len(args) < 1 {
		fmt.Fprintln(os.Stderr, "usage: steop state <get|set|set-phase|clear-phase|incr|reset|delete> ...")
		os.Exit(2)
	}
	c, err := client.NewFromConfig()
	if err != nil {
		fmt.Fprintf(os.Stderr, "state: client init: %v\n", err)
		os.Exit(1)
	}
	if globalProjectDir != "" {
		c = c.WithRequestContext("", globalProjectDir)
	}
	db, err := openStoreDB()
	if err != nil {
		fmt.Fprintf(os.Stderr, "state: open db: %v\n", err)
		os.Exit(1)
	}
	defer db.Close()
	ctx := context.Background()

	switch args[0] {
	case "get":
		if len(args) < 2 {
			fmt.Fprintln(os.Stderr, "usage: steop state get <session>")
			os.Exit(2)
		}
		id, err := identFor(c, args[1])
		if err != nil {
			fmt.Fprintf(os.Stderr, "state get: %v\n", err)
			os.Exit(2)
		}
		s, err := db.StateGet(ctx, id)
		if err != nil {
			fmt.Fprintf(os.Stderr, "state get: %v\n", err)
			os.Exit(1)
		}
		writeJSON(sessionAsJSON(c, s))
	case "set":
		if len(args) < 3 {
			fmt.Fprintln(os.Stderr, "usage: steop state set <session> <json>")
			os.Exit(2)
		}
		id, err := identFor(c, args[1])
		if err != nil {
			fmt.Fprintf(os.Stderr, "state set: %v\n", err)
			os.Exit(2)
		}
		var data map[string]interface{}
		if err := json.Unmarshal([]byte(args[2]), &data); err != nil {
			fmt.Fprintf(os.Stderr, "state set: parse json: %v\n", err)
			os.Exit(2)
		}
		raw, _ := json.Marshal(data)
		s, err := db.StatePut(ctx, id, raw, true)
		if err != nil {
			fmt.Fprintf(os.Stderr, "state set: %v\n", err)
			os.Exit(1)
		}
		writeJSON(sessionAsJSON(c, s))
	case "incr":
		if len(args) < 3 {
			fmt.Fprintln(os.Stderr, "usage: steop state incr <session> <counter> [delta]")
			os.Exit(2)
		}
		delta := int64(1)
		if len(args) >= 4 {
			d, err := strconv.ParseInt(args[3], 10, 64)
			if err != nil {
				fmt.Fprintf(os.Stderr, "state incr: parse delta: %v\n", err)
				os.Exit(2)
			}
			delta = d
		}
		id, err := identFor(c, args[1])
		if err != nil {
			fmt.Fprintf(os.Stderr, "state incr: %v\n", err)
			os.Exit(2)
		}
		v, err := db.StateIncr(ctx, id, args[2], delta)
		if err != nil {
			fmt.Fprintf(os.Stderr, "state incr: %v\n", err)
			os.Exit(1)
		}
		writeJSON(map[string]interface{}{"counter": args[2], "value": v})
	case "reset":
		if len(args) < 3 {
			fmt.Fprintln(os.Stderr, "usage: steop state reset <session> <counter> [value]")
			os.Exit(2)
		}
		value := int64(0)
		if len(args) >= 4 {
			v, err := strconv.ParseInt(args[3], 10, 64)
			if err != nil {
				fmt.Fprintf(os.Stderr, "state reset: parse value: %v\n", err)
				os.Exit(2)
			}
			value = v
		}
		id, err := identFor(c, args[1])
		if err != nil {
			fmt.Fprintf(os.Stderr, "state reset: %v\n", err)
			os.Exit(2)
		}
		v, err := db.StateReset(ctx, id, args[2], value)
		if err != nil {
			fmt.Fprintf(os.Stderr, "state reset: %v\n", err)
			os.Exit(1)
		}
		writeJSON(map[string]interface{}{"counter": args[2], "value": v})
	case "delete":
		if len(args) < 2 {
			fmt.Fprintln(os.Stderr, "usage: steop state delete <session>")
			os.Exit(2)
		}
		id, err := identFor(c, args[1])
		if err != nil {
			fmt.Fprintf(os.Stderr, "state delete: %v\n", err)
			os.Exit(2)
		}
		deleted, err := db.StateDelete(ctx, id)
		if err != nil {
			fmt.Fprintf(os.Stderr, "state delete: %v\n", err)
			os.Exit(1)
		}
		writeJSON(map[string]interface{}{"deleted": deleted})
	case "set-phase":
		// usage: steop state set-phase <phase> [--mode <mode>]
		if len(args) < 2 {
			fmt.Fprintln(os.Stderr, "usage: steop state set-phase <phase> [--mode <mode>]")
			os.Exit(2)
		}
		phase := args[1]
		mode := ""
		for i := 2; i < len(args); i++ {
			if args[i] == "--mode" && i+1 < len(args) {
				mode = args[i+1]
				i++
			}
		}
		sid := globalSessionID
		if sid == "" {
			return
		}
		id, err := identFor(c, sid)
		if err != nil {
			return
		}
		data := map[string]interface{}{"phase": phase}
		if mode != "" {
			data["mode"] = mode
		}
		raw, _ := json.Marshal(data)
		if _, err := db.StatePut(ctx, id, raw, true); err != nil {
			fmt.Fprintf(os.Stderr, "state set-phase: %v\n", err)
			return
		}

	case "clear-phase":
		sid := globalSessionID
		if sid == "" {
			return
		}
		id, err := identFor(c, sid)
		if err != nil {
			return
		}
		raw, _ := json.Marshal(map[string]interface{}{"phase": "", "mode": ""})
		if _, err := db.StatePut(ctx, id, raw, true); err != nil {
			fmt.Fprintf(os.Stderr, "state clear-phase: %v\n", err)
			return
		}

	default:
		fmt.Fprintf(os.Stderr, "unknown state subcommand: %s\n", args[0])
		os.Exit(2)
	}
}

func writeJSON(v interface{}) {
	b, err := json.MarshalIndent(v, "", "  ")
	if err != nil {
		fmt.Fprintf(os.Stderr, "marshal: %v\n", err)
		os.Exit(1)
	}
	os.Stdout.Write(b)
	os.Stdout.Write([]byte("\n"))
}

// openStoreDB opens the steop SQLite DB at the resolved path, creating and
// migrating it as needed. Callers must Close() it.
func openStoreDB() (*store.DB, error) {
	path, err := datadir.DBPath()
	if err != nil {
		return nil, err
	}
	return store.Open(path)
}

// identFor builds a 3-segment store.Identity using the client's host /
// project_dir and the given session id. Returns an error if the client hasn't
// resolved its own identity.
func identFor(c *client.Client, sessionID string) (store.Identity, error) {
	if c.Host() == "" {
		return store.Identity{}, fmt.Errorf("host is unknown")
	}
	if c.ProjectDir() == "" {
		return store.Identity{}, fmt.Errorf("project_dir is unknown (set CLAUDE_PROJECT_DIR)")
	}
	return store.Identity{
		Host:       c.Host(),
		ProjectDir: c.ProjectDir(),
		SessionID:  sessionID,
	}, nil
}

// sessionAsJSON marshals a store.Session into the shape the CLI and watcher
// have historically consumed (with composite `id`, decoded `data`, decoded
// `counters`). Returns nil when sess is nil.
func sessionAsJSON(c *client.Client, sess *store.Session) interface{} {
	if sess == nil {
		return nil
	}
	var data map[string]interface{}
	if len(sess.Data) > 0 {
		_ = json.Unmarshal(sess.Data, &data)
	}
	var counters map[string]int64
	if len(sess.Counters) > 0 {
		_ = json.Unmarshal(sess.Counters, &counters)
	}
	out := map[string]interface{}{
		"id":             client.ComposeSessionID(sess.Host, sess.ProjectDir, sess.SessionID),
		"state":          sess.State,
		"started_at":     sess.StartedAt,
		"last_active_at": sess.LastActiveAt,
		"data":           data,
		"counters":       counters,
	}
	if sess.StoppedAt != nil {
		out["stopped_at"] = *sess.StoppedAt
	} else {
		out["stopped_at"] = nil
	}
	return out
}
