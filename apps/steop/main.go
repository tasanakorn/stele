package main

import (
	"fmt"
	"os"
	"strings"
)

// Global identity overrides injected by the PreToolUse hook via
// --x-session-id=<val> and --x-project-dir=<val> flags.
var (
	globalSessionID  string
	globalProjectDir string
)

func main() {
	defer func() {
		if r := recover(); r != nil {
			fmt.Fprintf(os.Stderr, "steop panic: %v\n", r)
			os.Exit(1)
		}
	}()

	parseGlobalFlags()

	if len(os.Args) < 2 {
		fmt.Fprintln(os.Stderr, "usage: steop <hook|state|storage|statusline|monitor|inspect|mailbox|version> ...")
		os.Exit(2)
	}
	switch os.Args[1] {
	case "hook":
		runHook(os.Args[2:])
	case "state":
		runState(os.Args[2:])
	case "storage":
		runStorage(os.Args[2:])
	case "statusline":
		runStatusline(os.Args[2:])
	case "monitor", "inspect":
		runMonitor(os.Args[2:])
	case "mailbox":
		runMailbox(os.Args[2:])
	case "version":
		runVersion()
	default:
		fmt.Fprintf(os.Stderr, "unknown subcommand: %s\n", os.Args[1])
		os.Exit(2)
	}
}

// parseGlobalFlags scans os.Args for --x-session-id=<val> and
// --x-project-dir=<val>, stores their values, and strips them from os.Args
// so subcommand handlers see clean arguments.
func parseGlobalFlags() {
	var cleaned []string
	for _, arg := range os.Args {
		switch {
		case strings.HasPrefix(arg, "--x-session-id="):
			globalSessionID = arg[len("--x-session-id="):]
		case strings.HasPrefix(arg, "--x-project-dir="):
			globalProjectDir = arg[len("--x-project-dir="):]
		default:
			cleaned = append(cleaned, arg)
		}
	}
	os.Args = cleaned
}
