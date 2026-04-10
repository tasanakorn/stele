package main

import (
	"fmt"
	"os"

	"github.com/tasanakorn/stele/apps/steop/internal/client"
	"github.com/tasanakorn/stele/apps/steop/internal/hooks"
	"github.com/tasanakorn/stele/apps/steop/internal/logging"
)

func runHook(args []string) {
	if len(args) < 1 {
		fmt.Fprintln(os.Stderr, "usage: steop hook <event>")
		os.Exit(2)
	}
	event := args[0]

	in, err := hooks.ReadInput(os.Stdin)
	if err != nil {
		logging.Errorf("read hook input: %v", err)
		os.Stdout.Write(hooks.Allow())
		return
	}
	// Fall back to CLI arg event name if input didn't include it.
	if in.HookEventName == "" {
		in.HookEventName = event
	}

	var out []byte
	switch event {
	case "PreToolUse":
		out = hooks.HandlePreToolUse(in)
	case "PostToolUse":
		c, cerr := client.NewFromConfig()
		if cerr != nil {
			logging.Debugf("client init failed: %v", cerr)
			out = hooks.Allow()
		} else {
			out = hooks.HandlePostToolUse(in, c)
		}
	case "Stop":
		c, cerr := client.NewFromConfig()
		if cerr != nil {
			logging.Debugf("client init failed: %v", cerr)
			out = hooks.Allow()
		} else {
			out = hooks.HandleStop(in, c)
		}
	default:
		out = hooks.Allow()
	}
	os.Stdout.Write(out)
}
