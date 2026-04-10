package hooks

import (
	"github.com/tasanakorn/stele/apps/steop/internal/logging"
)

// HandleUserPromptSubmit records the current session id to the sentinel file
// so skills can discover it without racing on hook state. Always returns
// Allow() — sentinel write failures are logged and swallowed.
func HandleUserPromptSubmit(in *HookInput) []byte {
	if in == nil || in.SessionID == "" {
		return Allow()
	}
	if err := WriteSentinel(in.SessionID); err != nil {
		logging.Debugf("user_prompt_submit sentinel write failed: %v", err)
	}
	return Allow()
}
