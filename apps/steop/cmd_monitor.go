package main

import (
	"encoding/json"
	"errors"
	"fmt"
	"os"
	"sort"
	"strconv"
	"strings"

	"github.com/tasanakorn/stele/apps/steop/internal/client"
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

	if len(positional) == 0 {
		sessions, err := c.SessionList("", "", "", limit)
		if err != nil {
			fmt.Fprintf(os.Stderr, "monitor: list: %v\n", err)
			os.Exit(1)
		}
		if jsonOut {
			writeJSON(sessions)
			return
		}
		printSessionsTable(sessions)
		return
	}

	id := c.SessionCompositeID(positional[0])
	state, err := c.SessionGet(id)
	if err != nil {
		if errors.Is(err, client.ErrNotFound) {
			fmt.Fprintf(os.Stderr, "monitor: session not found: %s\n", id)
			os.Exit(1)
		}
		fmt.Fprintf(os.Stderr, "monitor: inspect: %v\n", err)
		os.Exit(1)
	}
	if jsonOut {
		writeJSON(state)
		return
	}
	printSessionInspect(state)
}

func printMonitorUsage() {
	fmt.Fprintln(os.Stderr, "usage: steop monitor [--json] [--limit=N] [<session_id>]")
	fmt.Fprintln(os.Stderr, "alias: steop inspect ...")
}

func printSessionsTable(sessions []client.Session) {
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
		fmt.Printf("%-60s  %-8s  %s\n", s.ID, state, s.LastActiveAt)
	}
}

func printSessionInspect(s *client.State) {
	fmt.Printf("id            : %s\n", s.ID)
	fmt.Printf("state         : %s\n", s.State)
	fmt.Printf("started_at    : %s\n", s.StartedAt)
	fmt.Printf("last_active_at: %s\n", s.LastActiveAt)

	fmt.Println("data:")
	if len(s.Data) == 0 {
		fmt.Println("  (empty)")
	} else {
		keys := make([]string, 0, len(s.Data))
		for k := range s.Data {
			keys = append(keys, k)
		}
		sort.Strings(keys)
		for _, k := range keys {
			v := s.Data[k]
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

	fmt.Println("counters:")
	if len(s.Counters) == 0 {
		fmt.Println("  (none)")
	} else {
		keys := make([]string, 0, len(s.Counters))
		for k := range s.Counters {
			keys = append(keys, k)
		}
		sort.Strings(keys)
		for _, k := range keys {
			fmt.Printf("  %s = %d\n", k, s.Counters[k])
		}
	}
}
