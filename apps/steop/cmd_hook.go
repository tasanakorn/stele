package main

import (
	"fmt"
	"os"

	"github.com/tasanakorn/stele/apps/steop/internal/client"
	"github.com/tasanakorn/stele/apps/steop/internal/datadir"
	"github.com/tasanakorn/stele/apps/steop/internal/hooks"
	"github.com/tasanakorn/stele/apps/steop/internal/logging"
	"github.com/tasanakorn/stele/apps/steop/internal/store"
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

	// PreToolUse / UserPromptSubmit / PermissionRequest are regex-only — no
	// store or client handle needed.
	switch event {
	case "UserPromptSubmit":
		os.Stdout.Write(hooks.HandleUserPromptSubmit(in))
		return
	case "PreToolUse":
		os.Stdout.Write(hooks.HandlePreToolUse(in, in.SessionID, os.Getenv("CLAUDE_PROJECT_DIR")))
		return
	case "PermissionRequest":
		os.Stdout.Write(hooks.HandlePermissionRequest(in))
		return
	}

	db := openHookDB()
	defer func() {
		if db != nil {
			db.Close()
		}
	}()

	newClient := func() *client.Client {
		c, err := client.NewFromConfig()
		if err != nil {
			logging.Debugf("client init failed: %v", err)
			return nil
		}
		c = c.WithRequestContext("", os.Getenv("CLAUDE_PROJECT_DIR"))
		if c.ProjectDir() == "" && in.SessionID != "" && db != nil {
			if resolveProjectDir(db, c, in.SessionID) {
				logging.Debugf("project_dir resolved from store for session %s", in.SessionID)
			}
		}
		return c
	}

	var out []byte
	switch event {
	case "PostToolUse":
		if c := newClient(); c != nil {
			out = hooks.HandlePostToolUse(in, db, c)
		} else {
			out = hooks.Allow()
		}
	case "Stop":
		if c := newClient(); c != nil {
			out = hooks.HandleStop(in, db, c)
		} else {
			out = hooks.Allow()
		}
	case "SessionStart":
		if c := newClient(); c != nil {
			out = hooks.HandleSessionStart(in, db, c)
		} else {
			out = hooks.Allow()
		}
	case "PostToolUseFailure":
		if c := newClient(); c != nil {
			out = hooks.HandlePostToolUseFailure(in, db, c)
		} else {
			out = hooks.Allow()
		}
	case "SubagentStart":
		if c := newClient(); c != nil {
			out = hooks.HandleSubagentStart(in, db, c)
		} else {
			out = hooks.Allow()
		}
	case "SubagentStop":
		if c := newClient(); c != nil {
			out = hooks.HandleSubagentStop(in, db, c)
		} else {
			out = hooks.Allow()
		}
	case "PreCompact":
		if c := newClient(); c != nil {
			out = hooks.HandlePreCompact(in, db, c)
		} else {
			out = hooks.Allow()
		}
	case "SessionEnd":
		if c := newClient(); c != nil {
			out = hooks.HandleSessionEnd(in, db, c)
		} else {
			out = hooks.Allow()
		}
	default:
		out = hooks.Allow()
	}
	os.Stdout.Write(out)
}

// openHookDB resolves the DB path and opens it. DB errors are logged and nil
// is returned — handlers must treat a nil db as a skip per PRD-020 §4.6.
func openHookDB() *store.DB {
	path, err := datadir.DBPath()
	if err != nil {
		logging.Debugf("hook: resolve db path: %v", err)
		return nil
	}
	db, err := store.Open(path)
	if err != nil {
		logging.Debugf("hook: open db %s: %v", path, err)
		return nil
	}
	return db
}
