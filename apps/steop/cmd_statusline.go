package main

import (
	"encoding/json"
	"errors"
	"fmt"
	"os"
	"strings"

	"github.com/tasanakorn/stele/apps/steop/internal/client"
)

// ANSI escape sequences used by the statusline renderer. One-shot print only —
// no cursor manipulation, no alternate screen.
//
// Bright set (9x) for dark-background legibility. We avoid dim (2m) and the
// 3x normal-intensity codes because both render with too little contrast on
// dark backgrounds.
const (
	statuslineReset         = "\x1b[0m"
	statuslineBold          = "\x1b[1m"
	statuslineWhite         = "\x1b[37m" // secondary text
	statuslineBrightWhite   = "\x1b[97m" // primary text
	statuslineBrightYellow  = "\x1b[93m"
	statuslineBrightCyan    = "\x1b[96m"
	statuslineBrightBlue    = "\x1b[94m"
	statuslineBrightGreen   = "\x1b[92m"
	statuslineBrightMagenta = "\x1b[95m"
	statuslineBrightRed     = "\x1b[91m"
)

// Phase → ANSI color for the phase token. Matches the agent palette in
// `plugins/steop/skills/st-flow/SKILL.md`, but uses bright variants for
// legibility on dark backgrounds.
var statuslinePhaseColors = map[string]string{
	"clarify":  statuslineBrightCyan,
	"research": statuslineBrightBlue,
	"plan":     statuslineBrightGreen,
	"execute":  statuslineBrightYellow,
	"validate": statuslineBrightMagenta,
}

type statuslineOpts struct {
	session   string
	noColor   bool
	jsonOut   bool
	line2Only bool
}

// runStatusline implements `steop statusline` — a two-line renderer for the
// Claude Code status bar.
//
// When stdin contains Claude Code's session JSON (and neither --json nor
// --line2-only is set), line 1 is printed first: model | project | git branch
// | context bar | cost or rate limits. Line 2 is always printed: the steop
// pipeline state fetched from the stele-server, or "idle"/"offline" on
// fallback.
//
// Always exits 0 — a broken statusline must not stall a Claude Code session.
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
		case a == "--line2-only":
			opts.line2Only = true
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

	var sess *Session
	if !opts.jsonOut && !opts.line2Only {
		if s, ok := parseSession(os.Stdin); ok {
			sess = s
		}
	}

	var (
		status    *client.Status
		statusMsg string
	)
	c, err := client.NewFromConfig()
	if err != nil {
		statusMsg = "offline"
	} else if sid, err := resolveStatuslineSession(c, opts.session); err != nil {
		if strings.Contains(err.Error(), "no sessions") {
			statusMsg = "idle"
		} else {
			statusMsg = "offline"
		}
	} else if s, err := c.StatusGet(sid); err != nil {
		if errors.Is(err, client.ErrNotFound) {
			statusMsg = "idle"
		} else {
			statusMsg = "offline"
		}
	} else {
		status = s
	}

	if opts.jsonOut {
		if status != nil {
			b, _ := json.Marshal(status)
			fmt.Println(string(b))
			return
		}
		fmt.Printf("{\"fallback\":%q}\n", statusMsg)
		return
	}

	line2 := formatStatuslineLine(status, statusMsg, opts.noColor)
	if sess != nil {
		if line1 := formatStatuslineLine1(sess, opts.noColor); line1 != "" {
			fmt.Println(line1)
		}
	}
	fmt.Println(line2)
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

// formatStatuslineLine renders the single steop line.
// Shape:  steop: [<mode>] <phase> <step>  loop=N tools=N retries=N
// On fallback:  steop: <msg>  (msg ∈ {"idle", "offline", "error"})
func formatStatuslineLine(s *client.Status, fallback string, noColor bool) string {
	prefix := colorize("steop:", statuslineBrightWhite+statuslineBold, noColor)

	if s == nil {
		msg := fallback
		if msg == "" {
			msg = "idle"
		}
		color := statuslineWhite
		if msg == "offline" || msg == "error" {
			color = statuslineBrightYellow
		}
		return prefix + " " + colorize(msg, color, noColor)
	}

	mode := s.Mode
	if mode == "" {
		mode = "idle"
	}
	phase := s.Phase
	if phase == "" {
		phase = "-"
	}
	step := s.Step
	if step == "" {
		step = "-"
	}

	modeTok := colorize("["+mode+"]", statuslineWhite, noColor)
	phaseColor := statuslinePhaseColors[strings.ToLower(phase)]
	if phaseColor == "" {
		phaseColor = statuslineBrightWhite
	}
	phaseTok := colorize(phase, phaseColor+statuslineBold, noColor)
	stepTok := colorize(step, statuslineBrightWhite, noColor)
	counters := fmt.Sprintf("loop=%d tools=%d retries=%d",
		s.LoopCount, s.ToolCalls, s.StepRetry)
	counterTok := colorize(counters, statuslineWhite, noColor)

	return fmt.Sprintf("%s %s %s %s  %s",
		prefix, modeTok, phaseTok, stepTok, counterTok)
}

func colorize(text, color string, noColor bool) string {
	if noColor || color == "" {
		return text
	}
	return color + text + statuslineReset
}

func printStatuslineUsage() {
	fmt.Fprintln(os.Stderr, "usage: steop statusline [--session=<id>] [--json] [--no-color] [--line2-only]")
	fmt.Fprintln(os.Stderr, "")
	fmt.Fprintln(os.Stderr, "  Two-line renderer for the Claude Code status bar.")
	fmt.Fprintln(os.Stderr, "  Configure in ~/.claude/settings.json:")
	fmt.Fprintln(os.Stderr, `    "statusLine": {"type": "command", "command": "steop statusline"}`)
	fmt.Fprintln(os.Stderr, "")
	fmt.Fprintln(os.Stderr, "  When stdin contains Claude Code's session JSON:")
	fmt.Fprintln(os.Stderr, "    Line 1: model | project | git branch | context bar | cost or rate limits")
	fmt.Fprintln(os.Stderr, "    Line 2: steop pipeline state (phase/step/counters)")
	fmt.Fprintln(os.Stderr, "")
	fmt.Fprintln(os.Stderr, "  Always exits 0 — a broken statusline must not stall a session.")
	fmt.Fprintln(os.Stderr, "")
	fmt.Fprintln(os.Stderr, "  --session=<id>  session to render (default: most recently updated)")
	fmt.Fprintln(os.Stderr, "  --json          emit JSON instead of a formatted line")
	fmt.Fprintln(os.Stderr, "  --no-color      disable ANSI colors (also honored via NO_COLOR env)")
	fmt.Fprintln(os.Stderr, "  --line2-only    skip line 1 even when stdin has session JSON")
}
