package hooks

import (
	"bytes"
	"encoding/json"
	"strings"
	"testing"
)

func makeBashInput(cmd string) *HookInput {
	ti, _ := json.Marshal(map[string]string{"command": cmd})
	return &HookInput{
		HookEventName: "PreToolUse",
		ToolName:      "Bash",
		ToolInput:     ti,
	}
}

func isDeny(out []byte) bool {
	return bytes.Contains(out, []byte(`"permissionDecision":"deny"`))
}

func TestPreToolUseBlocksDangerous(t *testing.T) {
	cases := []string{
		"git push --force",
		"git push -f origin feature",
		"git push origin main",
		"git push origin master",
		"rm -rf /",
		"rm -rf ~",
		"rm -rf ~/",
		"rm -rf $HOME",
		"cd /tmp && git push --force",
		"cd /tmp; rm -rf /",
	}
	for _, c := range cases {
		t.Run(c, func(t *testing.T) {
			out := HandlePreToolUse(makeBashInput(c), "", "")
			if !isDeny(out) {
				t.Errorf("expected deny for command %q, got %s", c, out)
			}
		})
	}
}

func TestPreToolUseAllowsSafe(t *testing.T) {
	cases := []string{
		"git push",
		"git push origin feature",
		"rm -rf ./build",
	}
	for _, c := range cases {
		t.Run(c, func(t *testing.T) {
			out := HandlePreToolUse(makeBashInput(c), "", "")
			if isDeny(out) {
				t.Errorf("expected allow for command %q, got deny: %s", c, out)
			}
		})
	}
}

// TestPreToolUseConservativeHomeRule documents that the `rm -rf ~` rule also
// blocks `rm -rf ~/Downloads/junk`. This is intentionally conservative —
// blocking anything that begins with the home directory prefix rather than the
// bare home directory. The test asserts this current behavior.
func TestPreToolUseConservativeHomeRule(t *testing.T) {
	out := HandlePreToolUse(makeBashInput("rm -rf ~/Downloads/junk"), "", "")
	if !isDeny(out) {
		t.Errorf("expected deny for conservative home rule; blueprint marks this as intentional. got: %s", out)
	}
}

func TestPreToolUseNonBashAllows(t *testing.T) {
	in := &HookInput{
		HookEventName: "PreToolUse",
		ToolName:      "Write",
	}
	out := HandlePreToolUse(in, "", "")
	if isDeny(out) {
		t.Errorf("expected allow for non-Bash tool, got %s", out)
	}
}

// --- Identity injection tests ---

func isUpdatedInput(out []byte) bool {
	return bytes.Contains(out, []byte(`"updatedInput"`))
}

func extractCommand(out []byte) string {
	var resp struct {
		HookSpecificOutput struct {
			UpdatedInput struct {
				Command string `json:"command"`
			} `json:"updatedInput"`
		} `json:"hookSpecificOutput"`
	}
	if err := json.Unmarshal(out, &resp); err != nil {
		return ""
	}
	return resp.HookSpecificOutput.UpdatedInput.Command
}

func TestPreToolUseInjectsIdentity(t *testing.T) {
	out := HandlePreToolUse(makeBashInput("steop state set-phase clarify --mode flow"), "d290f1ee-1234", "/tmp/proj")
	if !isUpdatedInput(out) {
		t.Fatalf("expected updatedInput, got %s", out)
	}
	cmd := extractCommand(out)
	if cmd == "" {
		t.Fatal("could not extract command from output")
	}
	if !bytes.Contains([]byte(cmd), []byte("--x-session-id=d290f1ee-1234")) {
		t.Errorf("missing --x-session-id in %q", cmd)
	}
	if !strings.Contains(cmd, "--x-project-dir='/tmp/proj'") {
		t.Errorf("missing --x-project-dir in %q", cmd)
	}
	trimmed := strings.TrimLeft(cmd, " \t")
	if !strings.HasPrefix(trimmed, "steop --x-session-id=") {
		t.Errorf("expected flags immediately after steop token, got: %s", cmd)
	}
}

func TestPreToolUseInjectsSessionIDOnly(t *testing.T) {
	out := HandlePreToolUse(makeBashInput("steop state get abc"), "sess-123", "")
	if !isUpdatedInput(out) {
		t.Fatalf("expected updatedInput, got %s", out)
	}
	cmd := extractCommand(out)
	if !bytes.Contains([]byte(cmd), []byte("--x-session-id=sess-123")) {
		t.Errorf("missing --x-session-id in %q", cmd)
	}
	if bytes.Contains([]byte(cmd), []byte("--x-project-dir")) {
		t.Errorf("unexpected --x-project-dir in %q", cmd)
	}
	trimmed := strings.TrimLeft(cmd, " \t")
	if !strings.HasPrefix(trimmed, "steop --x-session-id=") {
		t.Errorf("expected flags immediately after steop token, got: %s", cmd)
	}
}

func TestPreToolUseNoInjectionWithoutIdentity(t *testing.T) {
	out := HandlePreToolUse(makeBashInput("steop state get abc"), "", "")
	if isUpdatedInput(out) {
		t.Errorf("expected plain allow, got updatedInput: %s", out)
	}
}

func TestPreToolUseNoInjectionNonSteop(t *testing.T) {
	out := HandlePreToolUse(makeBashInput("echo hello"), "sess-123", "/tmp/proj")
	if isUpdatedInput(out) {
		t.Errorf("expected plain allow for non-steop command, got updatedInput: %s", out)
	}
}

func TestPreToolUseRespectsExistingFlags(t *testing.T) {
	out := HandlePreToolUse(makeBashInput("steop state get abc --x-session-id=existing"), "new-sess", "/tmp/proj")
	if isUpdatedInput(out) {
		t.Errorf("expected plain allow when --x-session-id already present, got updatedInput: %s", out)
	}
}

func TestPreToolUseChainedCommands(t *testing.T) {
	out := HandlePreToolUse(makeBashInput("steop state set-phase clarify && steop state clear-phase"), "sess-1", "/proj")
	if !isUpdatedInput(out) {
		t.Fatalf("expected updatedInput for chained commands, got %s", out)
	}
	cmd := extractCommand(out)
	// Both steop segments should have flags injected
	parts := bytes.Split([]byte(cmd), []byte("&&"))
	if len(parts) != 2 {
		t.Fatalf("expected 2 parts around &&, got %d in %q", len(parts), cmd)
	}
	for i, part := range parts {
		if !bytes.Contains(part, []byte("--x-session-id=sess-1")) {
			t.Errorf("segment %d missing --x-session-id: %q", i, string(part))
		}
		trimmedSegment := strings.TrimLeft(string(part), " \t")
		if !strings.HasPrefix(trimmedSegment, "steop --x-session-id=") {
			t.Errorf("segment %d: expected flags immediately after steop token, got: %s", i, string(part))
		}
	}
}

func TestPreToolUseSubstringNoMatch(t *testing.T) {
	out := HandlePreToolUse(makeBashInput("my-steop-wrapper do-thing"), "sess-1", "/proj")
	if isUpdatedInput(out) {
		t.Errorf("should not inject into steop-as-substring command: %s", out)
	}
}

func TestPreToolUseNoInjectionWhenSteopIsArgument(t *testing.T) {
	cases := []string{
		"echo steop",
		"ls -la steop",
		"cat steop/README.md",
		"grep steop file.txt",
	}
	for _, c := range cases {
		t.Run(c, func(t *testing.T) {
			out := HandlePreToolUse(makeBashInput(c), "sess-1", "/proj")
			if isUpdatedInput(out) {
				t.Errorf("should not inject when steop is an argument: %s", out)
			}
		})
	}
}

func TestPreToolUseProjectDirWithSpaces(t *testing.T) {
	out := HandlePreToolUse(makeBashInput("steop state get abc"), "sess-1", "/Users/my user/project")
	if !isUpdatedInput(out) {
		t.Fatalf("expected updatedInput, got %s", out)
	}
	cmd := extractCommand(out)
	// Project dir should be single-quoted for shell safety
	if !strings.Contains(cmd, "--x-project-dir='/Users/my user/project'") {
		t.Errorf("project dir not properly quoted in %q", cmd)
	}
	// Verify --x-project-dir appears before "state get" subcommand
	projectDirIdx := strings.Index(cmd, "--x-project-dir=")
	stateGetIdx := strings.Index(cmd, "state get")
	if projectDirIdx < 0 || stateGetIdx < 0 || projectDirIdx > stateGetIdx {
		t.Errorf("--x-project-dir must appear before state get subcommand; got: %s", cmd)
	}
}

func TestPreToolUseFlagsBeforeRedirection(t *testing.T) {
	in := &HookInput{
		ToolName:  "Monitor",
		ToolInput: json.RawMessage(`{"command":"steop mailbox watch > /tmp/out.log 2>&1"}`),
	}
	out := HandlePreToolUse(in, "sess-x", "/proj")
	got := extractCommand(out)
	// Flags must appear before any redirection.
	flagsAt := strings.Index(got, "--x-session-id=")
	redirAt := strings.Index(got, ">")
	if flagsAt < 0 || redirAt < 0 || flagsAt > redirAt {
		t.Errorf("flags must appear before redirection; got: %s", got)
	}
	if !strings.Contains(got, "--x-session-id=sess-x") ||
		!strings.Contains(got, "--x-project-dir='/proj'") {
		t.Errorf("missing identity flags in: %s", got)
	}
}

func TestPreToolUseFlagsAfterEnvVarPrefix(t *testing.T) {
	in := &HookInput{
		ToolName:  "Bash",
		ToolInput: json.RawMessage(`{"command":"FOO=1 BAR=baz steop run"}`),
	}
	out := HandlePreToolUse(in, "s", "/p")
	got := extractCommand(out)
	want := "FOO=1 BAR=baz steop --x-session-id=s --x-project-dir='/p' run"
	if got != want {
		t.Errorf("env-var prefix placement wrong.\n  want: %s\n  got:  %s", want, got)
	}
}
