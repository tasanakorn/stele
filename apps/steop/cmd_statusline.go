package main

import (
	"encoding/json"
	"errors"
	"fmt"
	"io"
	"os"
	"strings"

	"github.com/tasanakorn/stele/apps/steop/internal/client"
)

// ANSI escape sequences used by the statusline renderer. The statusline is a
// one-shot print — no cursor manipulation, no alternate screen.
const (
	statuslineReset = "\x1b[0m"
	statuslineDim   = "\x1b[2m"
)

var statuslinePhaseColors = map[string]string{
	"clarify":  "\x1b[36m", // cyan
	"research": "\x1b[34m", // blue
	"plan":     "\x1b[32m", // green
	"execute":  "\x1b[33m", // yellow
	"validate": "\x1b[35m", // magenta
}

type statuslineOpts struct {
	session string
	noColor bool
	jsonOut bool
}

// claudeStdin is the payload Claude Code writes to the statusline command on
// stdin. We currently do not use any fields, but draining stdin prevents the
// caller from getting EPIPE, and keeping the struct documents the contract.
type claudeStdin struct {
	Model struct {
		DisplayName string `json:"display_name"`
	} `json:"model"`
	Directory string `json:"directory"`
}

// runStatusline implements `steop statusline` — a one-shot renderer designed
// for Claude Code's native `statusLine` setting. It reads an optional JSON
// payload from stdin (Claude Code writes session context there), looks up the
// most recently updated steop session on stele-server, and prints a single
// line describing the current phase, step, and counters. It never exits
// non-zero — a broken statusline must not stall a Claude Code session.
func runStatusline(args []string) {
	opts := statuslineOpts{}
	for _, a := range args {
		switch {
		case a == "-h" || a == "--help":
			printStatuslineUsage()
			return
		case a == "--no-color":
			opts.noColor = true
		case a == "--json":
			opts.jsonOut = true
		case strings.HasPrefix(a, "--session="):
			opts.session = a[len("--session="):]
		default:
			fmt.Fprintf(os.Stderr, "statusline: unknown argument: %q\n", a)
			os.Exit(2)
		}
	}

	if os.Getenv("NO_COLOR") != "" {
		opts.noColor = true
	}

	// Drain Claude Code's JSON payload so the caller sees a clean EOF.
	_ = readClaudeStdin()

	c, err := client.NewFromConfig()
	if err != nil {
		printStatuslineFallback(opts, "steop offline")
		return
	}

	sid, err := resolveStatuslineSession(c, opts.session)
	if err != nil {
		printStatuslineFallback(opts, "steop idle")
		return
	}

	status, err := c.StatusGet(sid)
	if err != nil {
		if errors.Is(err, client.ErrNotFound) {
			printStatuslineFallback(opts, "steop idle")
			return
		}
		printStatuslineFallback(opts, "steop (error)")
		return
	}

	if opts.jsonOut {
		b, _ := json.Marshal(status)
		fmt.Println(string(b))
		return
	}
	fmt.Println(formatStatuslineLine(status, opts.noColor))
}

// readClaudeStdin reads Claude Code's stdin payload if stdin is a pipe. When
// invoked from a TTY (manual smoke test) the function returns nil without
// blocking.
func readClaudeStdin() *claudeStdin {
	fi, err := os.Stdin.Stat()
	if err != nil {
		return nil
	}
	if (fi.Mode() & os.ModeCharDevice) != 0 {
		return nil
	}
	data, err := io.ReadAll(os.Stdin)
	if err != nil || len(data) == 0 {
		return nil
	}
	var s claudeStdin
	if json.Unmarshal(data, &s) != nil {
		return nil
	}
	return &s
}

func resolveStatuslineSession(c *client.Client, wanted string) (string, error) {
	if wanted != "" {
		return wanted, nil
	}
	sessions, err := c.SessionsList(1)
	if err != nil {
		return "", err
	}
	if len(sessions) == 0 {
		return "", errors.New("no sessions")
	}
	return sessions[0].SessionID, nil
}

func formatStatuslineLine(s *client.Status, noColor bool) string {
	mode := s.Mode
	if mode == "" {
		mode = "-"
	}
	phase := s.Phase
	if phase == "" {
		phase = "-"
	}
	step := s.Step
	if step == "" {
		step = "-"
	}
	if noColor {
		return fmt.Sprintf("[%s] %s %s loop=%d tools=%d retries=%d",
			mode, phase, step, s.LoopCount, s.ToolCalls, s.StepRetry)
	}
	color := statuslinePhaseColors[strings.ToLower(phase)]
	return fmt.Sprintf("%s[%s]%s %s%s%s %s %sloop=%d tools=%d retries=%d%s",
		statuslineDim, mode, statuslineReset,
		color, phase, statuslineReset,
		step,
		statuslineDim, s.LoopCount, s.ToolCalls, s.StepRetry, statuslineReset)
}

func printStatuslineFallback(opts statuslineOpts, msg string) {
	if opts.jsonOut {
		fmt.Printf("{\"fallback\":%q}\n", msg)
		return
	}
	if opts.noColor {
		fmt.Println(msg)
		return
	}
	fmt.Printf("%s%s%s\n", statuslineDim, msg, statuslineReset)
}

func printStatuslineUsage() {
	fmt.Fprintln(os.Stderr, "usage: steop statusline [--session=<id>] [--json] [--no-color]")
	fmt.Fprintln(os.Stderr, "")
	fmt.Fprintln(os.Stderr, "  One-shot renderer for Claude Code's native statusLine setting.")
	fmt.Fprintln(os.Stderr, "  Reads a JSON payload from stdin (optional; written by Claude Code),")
	fmt.Fprintln(os.Stderr, "  then prints a single line describing the current steop session phase,")
	fmt.Fprintln(os.Stderr, "  step, and counters. Exits 0 even on error.")
	fmt.Fprintln(os.Stderr, "")
	fmt.Fprintln(os.Stderr, "  --session=<id>  session to render (default: most recently updated)")
	fmt.Fprintln(os.Stderr, "  --json          emit JSON instead of a formatted line")
	fmt.Fprintln(os.Stderr, "  --no-color      disable ANSI colors (also honored via NO_COLOR env)")
}
