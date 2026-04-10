package main

import (
	"encoding/json"
	"fmt"
	"os"
	"strconv"

	"github.com/tasanakorn/stele/apps/steop/internal/client"
)

func runState(args []string) {
	if len(args) < 1 {
		fmt.Fprintln(os.Stderr, "usage: steop state <get|set|incr|reset|delete> ...")
		os.Exit(2)
	}
	c, err := client.NewFromConfig()
	if err != nil {
		fmt.Fprintf(os.Stderr, "state: client init: %v\n", err)
		os.Exit(1)
	}

	switch args[0] {
	case "get":
		if len(args) < 2 {
			fmt.Fprintln(os.Stderr, "usage: steop state get <session>")
			os.Exit(2)
		}
		s, err := c.StateGet(args[1])
		if err != nil {
			fmt.Fprintf(os.Stderr, "state get: %v\n", err)
			os.Exit(1)
		}
		writeJSON(s)
	case "set":
		if len(args) < 3 {
			fmt.Fprintln(os.Stderr, "usage: steop state set <session> <json>")
			os.Exit(2)
		}
		var data map[string]interface{}
		if err := json.Unmarshal([]byte(args[2]), &data); err != nil {
			fmt.Fprintf(os.Stderr, "state set: parse json: %v\n", err)
			os.Exit(2)
		}
		s, err := c.StatePut(args[1], data, true)
		if err != nil {
			fmt.Fprintf(os.Stderr, "state set: %v\n", err)
			os.Exit(1)
		}
		writeJSON(s)
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
		v, err := c.CounterIncr(args[1], args[2], delta)
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
		v, err := c.CounterReset(args[1], args[2], value)
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
		deleted, err := c.StateDelete(args[1])
		if err != nil {
			fmt.Fprintf(os.Stderr, "state delete: %v\n", err)
			os.Exit(1)
		}
		writeJSON(map[string]interface{}{"deleted": deleted})
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
