package main

import (
	"os"
	"testing"
)

func TestParseGlobalFlagsPublicAliases(t *testing.T) {
	cases := []struct {
		name            string
		args            []string
		wantSessionID   string
		wantProjectDir  string
		wantArgStripped bool
	}{
		{
			name:           "--session-id sets globalSessionID",
			args:           []string{"steop", "--session-id=abc", "identity"},
			wantSessionID:  "abc",
			wantProjectDir: "",
		},
		{
			name:           "--project-dir sets globalProjectDir",
			args:           []string{"steop", "--project-dir=/p", "identity"},
			wantSessionID:  "",
			wantProjectDir: "/p",
		},
		{
			name:          "--x-session-id then --session-id: last wins",
			args:          []string{"steop", "--x-session-id=A", "--session-id=B", "identity"},
			wantSessionID: "B",
		},
		{
			name:          "--session-id then --x-session-id: last wins",
			args:          []string{"steop", "--session-id=A", "--x-session-id=B", "identity"},
			wantSessionID: "B",
		},
	}

	for _, tc := range cases {
		t.Run(tc.name, func(t *testing.T) {
			// Save and restore state.
			origArgs := os.Args
			origSessionID := globalSessionID
			origProjectDir := globalProjectDir
			defer func() {
				os.Args = origArgs
				globalSessionID = origSessionID
				globalProjectDir = origProjectDir
			}()

			globalSessionID = ""
			globalProjectDir = ""
			os.Args = tc.args

			parseGlobalFlags()

			if tc.wantSessionID != "" && globalSessionID != tc.wantSessionID {
				t.Errorf("globalSessionID = %q, want %q", globalSessionID, tc.wantSessionID)
			}
			if tc.wantProjectDir != "" && globalProjectDir != tc.wantProjectDir {
				t.Errorf("globalProjectDir = %q, want %q", globalProjectDir, tc.wantProjectDir)
			}

			// Verify global flags were stripped from os.Args.
			for _, arg := range os.Args {
				if arg == "--session-id=abc" || arg == "--session-id=A" || arg == "--session-id=B" ||
					arg == "--project-dir=/p" || arg == "--x-session-id=A" || arg == "--x-session-id=B" {
					t.Errorf("flag %q was not stripped from os.Args: %v", arg, os.Args)
				}
			}
		})
	}
}
