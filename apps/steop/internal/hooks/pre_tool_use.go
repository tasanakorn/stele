package hooks

import (
	"encoding/json"
	"regexp"
	"strings"
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

// steopLeadRe matches "steop" as the first command token in a shell segment
// (after trimming whitespace), allowing leading env-var assignments.
// Anchored to ^ so it only matches at the start of a segment, not as an
// argument to another command (e.g. "echo steop" must NOT match).
var steopLeadRe = regexp.MustCompile(`^(?:\w+=\S*\s+)*steop(?:\s|$)`)

// shellSplitRe splits a command on shell operators while capturing delimiters.
var shellSplitRe = regexp.MustCompile(`(&&|\|\||[;|])`)

type bashToolInput struct {
	Command string `json:"command"`
}

// HandlePreToolUse inspects a Bash command for dangerous patterns, then injects
// identity flags into steop invocations. sessionID and projectDir come from the
// hook's stdin JSON and CLAUDE_PROJECT_DIR env respectively.
func HandlePreToolUse(in *HookInput, sessionID, projectDir string) []byte {
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
	// Safety checks run first — deny takes precedence over injection.
	for _, p := range dangerousPatterns {
		if p.re.MatchString(b.Command) {
			return DenyPreToolUse(p.reason)
		}
	}

	// Identity injection: append --x-session-id and --x-project-dir to steop segments.
	rewritten := injectIdentity(b.Command, sessionID, projectDir)
	if rewritten == b.Command {
		return Allow()
	}
	return AllowWithUpdatedInput(map[string]interface{}{"command": rewritten})
}

// isSteopSegment returns true if the trimmed segment starts with "steop" as
// the first command token (after optional env-var assignments).
func isSteopSegment(segment string) bool {
	return steopLeadRe.MatchString(strings.TrimLeft(segment, " \t"))
}

// injectIdentity appends identity flags to each steop invocation in cmd.
// Returns the original string unchanged if no injection is needed.
func injectIdentity(cmd, sessionID, projectDir string) string {
	if sessionID == "" && projectDir == "" {
		return cmd
	}
	// Don't inject if flags are already present (respect explicit overrides).
	if strings.Contains(cmd, "--x-session-id") || strings.Contains(cmd, "--x-project-dir") {
		return cmd
	}

	// Split on shell operators, inject into each steop-leading segment.
	parts := shellSplitRe.Split(cmd, -1)
	delims := shellSplitRe.FindAllString(cmd, -1)

	injected := false
	suffix := buildIdentitySuffix(sessionID, projectDir)

	var result strings.Builder
	for i, part := range parts {
		if isSteopSegment(part) {
			part = strings.TrimRight(part, " \t") + suffix
			injected = true
		}
		result.WriteString(part)
		if i < len(delims) {
			result.WriteString(" ")
			result.WriteString(delims[i])
			result.WriteString(" ")
		}
	}
	if !injected {
		return cmd
	}
	return result.String()
}

// shellQuote wraps a value in single quotes for safe shell embedding.
func shellQuote(s string) string {
	return "'" + strings.ReplaceAll(s, "'", "'\\''") + "'"
}

func buildIdentitySuffix(sessionID, projectDir string) string {
	var sb strings.Builder
	if sessionID != "" {
		sb.WriteString(" --x-session-id=")
		sb.WriteString(sessionID)
	}
	if projectDir != "" {
		sb.WriteString(" --x-project-dir=")
		sb.WriteString(shellQuote(projectDir))
	}
	return sb.String()
}
