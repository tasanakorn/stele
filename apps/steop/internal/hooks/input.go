package hooks

import (
	"encoding/json"
	"fmt"
	"io"
)

type HookInput struct {
	SessionID      string          `json:"session_id"`
	TranscriptPath string          `json:"transcript_path"`
	Cwd            string          `json:"cwd"`
	PermissionMode string          `json:"permission_mode"`
	HookEventName  string          `json:"hook_event_name"`
	ToolName       string          `json:"tool_name,omitempty"`
	ToolInput      json.RawMessage `json:"tool_input,omitempty"`
	ToolResponse   json.RawMessage `json:"tool_response,omitempty"`
	ToolUseID      string          `json:"tool_use_id,omitempty"`
	Prompt               string          `json:"prompt,omitempty"`
	StopHookActive       bool            `json:"stop_hook_active,omitempty"`
	LastAssistantMessage string          `json:"last_assistant_message,omitempty"`
}

// ReadInput decodes a HookInput JSON object from r.
func ReadInput(r io.Reader) (*HookInput, error) {
	data, err := io.ReadAll(r)
	if err != nil {
		return nil, fmt.Errorf("read hook input: %w", err)
	}
	if len(data) == 0 {
		return &HookInput{}, nil
	}
	var in HookInput
	if err := json.Unmarshal(data, &in); err != nil {
		return nil, fmt.Errorf("decode hook input: %w", err)
	}
	return &in, nil
}
