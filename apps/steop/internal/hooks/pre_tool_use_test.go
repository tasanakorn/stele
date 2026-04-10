package hooks

import (
	"bytes"
	"encoding/json"
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
			out := HandlePreToolUse(makeBashInput(c))
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
			out := HandlePreToolUse(makeBashInput(c))
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
	out := HandlePreToolUse(makeBashInput("rm -rf ~/Downloads/junk"))
	if !isDeny(out) {
		t.Errorf("expected deny for conservative home rule; blueprint marks this as intentional. got: %s", out)
	}
}

func TestPreToolUseNonBashAllows(t *testing.T) {
	in := &HookInput{
		HookEventName: "PreToolUse",
		ToolName:      "Write",
	}
	out := HandlePreToolUse(in)
	if isDeny(out) {
		t.Errorf("expected allow for non-Bash tool, got %s", out)
	}
}
