package hooks

import (
	"os"
	"path/filepath"
	"regexp"
	"strings"

	"github.com/tasanakorn/stele/apps/steop/internal/logging"
)

var skillTriggers = []struct {
	re   *regexp.Regexp
	name string
}{
	{regexp.MustCompile(`(?i)^/?(steop:)?st-flow\b|^flow:`), "st-flow"},
	{regexp.MustCompile(`(?i)^/?(steop:)?st-clarify\b|^clarify:`), "st-clarify"},
	{regexp.MustCompile(`(?i)^/?(steop:)?st-research\b|^research:`), "st-research"},
	{regexp.MustCompile(`(?i)^/?(steop:)?st-plan\b|^plan:`), "st-plan"},
	{regexp.MustCompile(`(?i)^/?(steop:)?st-execute\b|^execute:`), "st-execute"},
	{regexp.MustCompile(`(?i)^/?(steop:)?st-validate\b|^validate:`), "st-validate"},
}

func loadSkillBody(name string) (string, bool) {
	root := os.Getenv("CLAUDE_PLUGIN_ROOT")
	if root == "" {
		return "", false
	}
	path := filepath.Join(root, "skills", name, "SKILL.md")
	data, err := os.ReadFile(path)
	if err != nil {
		return "", false
	}
	return string(data), true
}

// HandleUserPromptSubmit checks the prompt for steop skill triggers and, when
// matched, injects the SKILL.md body as additional context for the model.
func HandleUserPromptSubmit(in *HookInput) []byte {
	if in == nil || in.SessionID == "" {
		return Allow()
	}
	prompt := strings.TrimSpace(in.Prompt)
	if prompt == "" {
		return Allow()
	}
	for _, t := range skillTriggers {
		if t.re.MatchString(prompt) {
			if body, ok := loadSkillBody(t.name); ok {
				return InjectUserPromptContext(body)
			}
			logging.Debugf("skill %s matched but body not found", t.name)
			break
		}
	}
	return Allow()
}
