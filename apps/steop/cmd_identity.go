package main

import (
	"encoding/json"
	"fmt"
	"os"

	"github.com/tasanakorn/stele/apps/steop/internal/client"
)

func runIdentity(args []string) {
	// ignore positional args
	_ = args
	c, err := client.NewFromConfig()
	if err != nil {
		fmt.Fprintf(os.Stderr, "identity: %v\n", err)
		os.Exit(1)
	}
	if globalProjectDir != "" {
		c = c.WithRequestContext("", globalProjectDir)
	}
	if globalSessionID != "" && c.ProjectDir() == "" {
		c.ResolveProjectDir(globalSessionID)
	}
	out := struct {
		SessionID          string `json:"session_id"`
		ProjectDir         string `json:"project_dir"`
		Host               string `json:"host"`
		SessionCompositeID string `json:"session_composite_id"`
		ProjectCompositeID string `json:"project_composite_id"`
	}{
		SessionID:          globalSessionID,
		ProjectDir:         c.ProjectDir(),
		Host:               c.Host(),
		ProjectCompositeID: c.ProjectID(),
	}
	if globalSessionID != "" {
		out.SessionCompositeID = c.SessionCompositeID(globalSessionID)
	}
	b, _ := json.MarshalIndent(out, "", "  ")
	os.Stdout.Write(append(b, '\n'))
}
