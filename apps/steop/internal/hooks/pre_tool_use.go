package hooks

import (
	"encoding/json"
	"regexp"
)

var dangerousPatterns = []struct {
	re     *regexp.Regexp
	reason string
}{
	{regexp.MustCompile(`(?m)(^|&&|;|\|)\s*git\s+push\s+(--force|-f)(\s|$)`), "blocked: git force push"},
	{regexp.MustCompile(`(?m)(^|&&|;|\|)\s*git\s+push\s+origin\s+(main|master)(\s|$)`), "blocked: direct push to main/master"},
	{regexp.MustCompile(`(?m)(^|&&|;|\|)\s*git\s+push\s+.*\s+(main|master)\s+.*(--force|-f)`), "blocked: forced push to main/master"},
	{regexp.MustCompile(`(?m)(^|&&|;|\|)\s*rm\s+-rf\s+/(\s|$)`), "blocked: rm -rf /"},
	{regexp.MustCompile(`(?m)(^|&&|;|\|)\s*rm\s+-rf\s+~(/|\s|$)`), "blocked: rm -rf home"},
	{regexp.MustCompile(`(?m)(^|&&|;|\|)\s*rm\s+-rf\s+\$HOME`), "blocked: rm -rf $HOME"},
}

type bashToolInput struct {
	Command string `json:"command"`
}

// HandlePreToolUse inspects a Bash command for dangerous patterns and returns
// a deny payload if one matches. Non-Bash tools always return Allow().
func HandlePreToolUse(in *HookInput) []byte {
	if in == nil || in.ToolName != "Bash" || len(in.ToolInput) == 0 {
		return Allow()
	}
	var b bashToolInput
	if err := json.Unmarshal(in.ToolInput, &b); err != nil {
		return Allow()
	}
	if b.Command == "" {
		return Allow()
	}
	for _, p := range dangerousPatterns {
		if p.re.MatchString(b.Command) {
			return DenyPreToolUse(p.reason)
		}
	}
	return Allow()
}
