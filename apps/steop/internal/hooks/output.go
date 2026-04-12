package hooks

import "encoding/json"

// DenyPreToolUse returns the JSON bytes for denying a PreToolUse tool call.
func DenyPreToolUse(reason string) []byte {
	out := map[string]interface{}{
		"hookSpecificOutput": map[string]interface{}{
			"hookEventName":            "PreToolUse",
			"permissionDecision":       "deny",
			"permissionDecisionReason": reason,
		},
	}
	b, _ := json.Marshal(out)
	return b
}

// InjectUserPromptContext returns bytes that inject context into the user prompt.
func InjectUserPromptContext(context string) []byte {
	out := map[string]interface{}{
		"hookSpecificOutput": map[string]interface{}{
			"hookEventName":     "UserPromptSubmit",
			"additionalContext": context,
		},
	}
	b, _ := json.Marshal(out)
	return b
}

// AllowWithUpdatedInput returns a PreToolUse allow response that rewrites
// the tool input. Used by identity injection to append flags to Bash commands.
func AllowWithUpdatedInput(toolInput map[string]interface{}) []byte {
	out := map[string]interface{}{
		"hookSpecificOutput": map[string]interface{}{
			"hookEventName":      "PreToolUse",
			"permissionDecision": "allow",
			"updatedInput":       toolInput,
		},
	}
	b, _ := json.Marshal(out)
	return b
}

// Allow returns the allow / no-op hook output bytes.
func Allow() []byte {
	return []byte("{}")
}
