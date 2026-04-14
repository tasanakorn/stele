package main

import (
	"context"
	"encoding/json"
	"fmt"
	"os"
	"sort"
	"strconv"
	"strings"

	"github.com/tasanakorn/stele/apps/steop/internal/client"
	"github.com/tasanakorn/stele/apps/steop/internal/store"
)

func runMonitor(args []string) {
	jsonOut := false
	limit := 0
	var positional []string
	for _, a := range args {
		switch {
		case a == "--json":
			jsonOut = true
		case a == "-h" || a == "--help":
			printMonitorUsage()
			return
		case strings.HasPrefix(a, "--limit="):
			v, err := strconv.Atoi(a[len("--limit="):])
			if err != nil || v <= 0 {
				fmt.Fprintf(os.Stderr, "monitor: invalid --limit value: %q\n", a[len("--limit="):])
				os.Exit(2)
			}
			limit = v
		default:
			positional = append(positional, a)
		}
	}

	c, err := client.NewFromConfig()
	if err != nil {
		fmt.Fprintf(os.Stderr, "monitor: client init: %v\n", err)
		os.Exit(1)
	}
	if globalProjectDir != "" {
		c = c.WithRequestContext("", globalProjectDir)
	}
	db, err := openStoreDB()
	if err != nil {
		fmt.Fprintf(os.Stderr, "monitor: open db: %v\n", err)
		os.Exit(1)
	}
	defer db.Close()
	ctx := context.Background()

	if len(positional) == 0 {
		sessions, err := db.SessionList(ctx, "", "", "", limit)
		if err != nil {
			fmt.Fprintf(os.Stderr, "monitor: list: %v\n", err)
			os.Exit(1)
		}
		if jsonOut {
			out := make([]interface{}, 0, len(sessions))
			for i := range sessions {
				out = append(out, sessionAsJSON(c, &sessions[i]))
			}
			writeJSON(out)
			return
		}
		printSessionsTable(sessions)
		return
	}

	id, err := identFor(c, positional[0])
	if err != nil {
		fmt.Fprintf(os.Stderr, "monitor: %v\n", err)
		os.Exit(1)
	}
	state, err := db.SessionGet(ctx, id)
	if err != nil {
		fmt.Fprintf(os.Stderr, "monitor: inspect: %v\n", err)
		os.Exit(1)
	}
	if state == nil {
		fmt.Fprintf(os.Stderr, "monitor: session not found: %s\n", client.ComposeSessionID(id.Host, id.ProjectDir, id.SessionID))
		os.Exit(1)
	}
	if jsonOut {
		writeJSON(sessionAsJSON(c, state))
		return
	}
	printSessionInspect(state)
}

func printMonitorUsage() {
	fmt.Fprintln(os.Stderr, "usage: steop monitor [--json] [--limit=N] [<session_id>]")
	fmt.Fprintln(os.Stderr, "alias: steop inspect ...")
}

func printSessionsTable(sessions []store.Session) {
	if len(sessions) == 0 {
		fmt.Println("(no sessions)")
		return
	}
	fmt.Printf("%-60s  %-8s  %s\n", "ID", "STATE", "LAST_ACTIVE")
	for _, s := range sessions {
		state := s.State
		if state == "" {
			state = "-"
		}
		id := client.ComposeSessionID(s.Host, s.ProjectDir, s.SessionID)
		fmt.Printf("%-60s  %-8s  %d\n", id, state, s.LastActiveAt)
	}
}

func printSessionInspect(s *store.Session) {
	id := client.ComposeSessionID(s.Host, s.ProjectDir, s.SessionID)
	fmt.Printf("id            : %s\n", id)
	fmt.Printf("state         : %s\n", s.State)
	fmt.Printf("started_at    : %d\n", s.StartedAt)
	fmt.Printf("last_active_at: %d\n", s.LastActiveAt)

	var data map[string]interface{}
	if len(s.Data) > 0 {
		_ = json.Unmarshal(s.Data, &data)
	}
	fmt.Println("data:")
	if len(data) == 0 {
		fmt.Println("  (empty)")
	} else {
		keys := make([]string, 0, len(data))
		for k := range data {
			keys = append(keys, k)
		}
		sort.Strings(keys)
		for _, k := range keys {
			v := data[k]
			switch vv := v.(type) {
			case string:
				fmt.Printf("  %s = %s\n", k, vv)
			case float64, bool, nil:
				fmt.Printf("  %s = %v\n", k, vv)
			default:
				b, _ := json.Marshal(vv)
				fmt.Printf("  %s = %s\n", k, string(b))
			}
		}
	}

	var counters map[string]int64
	if len(s.Counters) > 0 {
		_ = json.Unmarshal(s.Counters, &counters)
	}
	fmt.Println("counters:")
	if len(counters) == 0 {
		fmt.Println("  (none)")
	} else {
		keys := make([]string, 0, len(counters))
		for k := range counters {
			keys = append(keys, k)
		}
		sort.Strings(keys)
		for _, k := range keys {
			fmt.Printf("  %s = %d\n", k, counters[k])
		}
	}
}
