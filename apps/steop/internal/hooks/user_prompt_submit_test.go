package hooks

import (
	"bytes"
	"os"
	"path/filepath"
	"testing"
)

func writeSkill(t *testing.T, root, name, body string) {
	t.Helper()
	dir := filepath.Join(root, "skills", name)
	if err := os.MkdirAll(dir, 0o755); err != nil {
		t.Fatalf("mkdir %s: %v", dir, err)
	}
	if err := os.WriteFile(filepath.Join(dir, "SKILL.md"), []byte(body), 0o644); err != nil {
		t.Fatalf("write skill body: %v", err)
	}
}

func isInject(out []byte) bool {
	return bytes.Contains(out, []byte(`"hookEventName":"UserPromptSubmit"`)) &&
		bytes.Contains(out, []byte(`"additionalContext"`))
}

func TestUserPromptSubmitInjectsStFlow(t *testing.T) {
	root := t.TempDir()
	writeSkill(t, root, "st-flow", "FLOW SKILL BODY")
	t.Setenv("CLAUDE_PLUGIN_ROOT", root)

	in := &HookInput{SessionID: "s1", Prompt: "/steop:st-flow help me"}
	out := HandleUserPromptSubmit(in)
	if !isInject(out) {
		t.Fatalf("expected inject output, got %s", out)
	}
	if !bytes.Contains(out, []byte("FLOW SKILL BODY")) {
		t.Errorf("expected flow skill body in output, got %s", out)
	}
}

func TestUserPromptSubmitFlowColonAlias(t *testing.T) {
	root := t.TempDir()
	writeSkill(t, root, "st-flow", "FLOW BODY")
	t.Setenv("CLAUDE_PLUGIN_ROOT", root)

	out := HandleUserPromptSubmit(&HookInput{SessionID: "s1", Prompt: "flow: rebuild"})
	if !isInject(out) {
		t.Fatalf("expected inject for 'flow:' alias, got %s", out)
	}
}

func TestUserPromptSubmitStPlanBareForm(t *testing.T) {
	root := t.TempDir()
	writeSkill(t, root, "st-plan", "PLAN BODY")
	t.Setenv("CLAUDE_PLUGIN_ROOT", root)

	out := HandleUserPromptSubmit(&HookInput{SessionID: "s1", Prompt: "st-plan do X"})
	if !isInject(out) {
		t.Fatalf("expected inject for 'st-plan', got %s", out)
	}
	if !bytes.Contains(out, []byte("PLAN BODY")) {
		t.Errorf("expected plan body in output, got %s", out)
	}
}

func TestUserPromptSubmitPlainPromptAllows(t *testing.T) {
	root := t.TempDir()
	writeSkill(t, root, "st-flow", "FLOW BODY")
	t.Setenv("CLAUDE_PLUGIN_ROOT", root)

	out := HandleUserPromptSubmit(&HookInput{SessionID: "s1", Prompt: "plain question with no prefix"})
	if isInject(out) {
		t.Errorf("expected allow, got inject: %s", out)
	}
}

func TestUserPromptSubmitNoPluginRootAllows(t *testing.T) {
	t.Setenv("CLAUDE_PLUGIN_ROOT", "")

	out := HandleUserPromptSubmit(&HookInput{SessionID: "s1", Prompt: "/steop:st-flow go"})
	if isInject(out) {
		t.Errorf("expected allow when CLAUDE_PLUGIN_ROOT unset, got inject: %s", out)
	}
}
