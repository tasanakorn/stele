package main

import (
	"fmt"
	"runtime/debug"
)

// runVersion prints the human-facing version const plus any VCS build
// metadata Go automatically embedded when the binary was compiled from a
// git checkout or fetched via the module proxy. The const is the marketing
// version (kept in sync with plugin.json); the VCS fields reveal the actual
// commit behind it so the two can be cross-checked.
func runVersion() {
	fmt.Println(Version)
	info, ok := debug.ReadBuildInfo()
	if !ok {
		return
	}
	for _, s := range info.Settings {
		switch s.Key {
		case "vcs.revision", "vcs.time", "vcs.modified":
			fmt.Printf("  %s: %s\n", s.Key, s.Value)
		}
	}
}
