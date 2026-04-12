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
}
