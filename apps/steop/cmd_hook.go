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

	newClient := func() *client.Client {
		c, err := client.NewFromConfig()
		if err != nil {
			logging.Debugf("client init failed: %v", err)
			return nil
		}
		c = c.WithRequestContext("", os.Getenv("CLAUDE_PROJECT_DIR"))
		if c.ProjectDir() == "" && in.SessionID != "" {
			if c.ResolveProjectDir(in.SessionID) {
				logging.Debugf("project_dir resolved from server for session %s", in.SessionID)
			}
		}
		return c
	}

	var out []byte
	switch event {
	case "UserPromptSubmit":
		out = hooks.HandleUserPromptSubmit(in)
	case "PreToolUse":
		out = hooks.HandlePreToolUse(in)
	case "PostToolUse":
		if c := newClient(); c != nil {
			out = hooks.HandlePostToolUse(in, c)
		} else {
			out = hooks.Allow()
		}
	case "Stop":
		if c := newClient(); c != nil {
			out = hooks.HandleStop(in, c)
		} else {
			out = hooks.Allow()
		}
	case "SessionStart":
		if c := newClient(); c != nil {
			out = hooks.HandleSessionStart(in, c)
		} else {
			out = hooks.Allow()
		}
	case "PermissionRequest":
		out = hooks.HandlePermissionRequest(in)
	case "PostToolUseFailure":
		if c := newClient(); c != nil {
			out = hooks.HandlePostToolUseFailure(in, c)
		} else {
			out = hooks.Allow()
		}
	case "SubagentStart":
		if c := newClient(); c != nil {
			out = hooks.HandleSubagentStart(in, c)
		} else {
			out = hooks.Allow()
		}
	case "SubagentStop":
		if c := newClient(); c != nil {
			out = hooks.HandleSubagentStop(in, c)
		} else {
			out = hooks.Allow()
		}
	case "PreCompact":
		if c := newClient(); c != nil {
			out = hooks.HandlePreCompact(in, c)
		} else {
			out = hooks.Allow()
		}
	case "SessionEnd":
		if c := newClient(); c != nil {
			out = hooks.HandleSessionEnd(in, c)
		} else {
			out = hooks.Allow()
		}
	default:
		out = hooks.Allow()
	}
	os.Stdout.Write(out)
}
