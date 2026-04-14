package main

import (
	"context"
	"encoding/json"
	"fmt"
	"os"
	"strings"

	"github.com/tasanakorn/stele/apps/steop/internal/client"
	"github.com/tasanakorn/stele/apps/steop/internal/datadir"
	"github.com/tasanakorn/stele/apps/steop/internal/store"
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
// pipeline state read from the local store, or "idle"/"offline" on fallback.
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

	// Always parse stdin to extract session_id for exact session lookup,
	// even in --line2-only mode where line 1 is not rendered.
	var sess *Session
	if s, ok := parseSession(os.Stdin); ok {
		sess = s
	}

	var (
		status    *store.Status
		statusMsg string
	)

	c, err := client.NewFromConfig()
	if err != nil {
		statusMsg = "offline"
	} else {
		status, statusMsg = loadStatuslineStatus(c, opts.session, sess)
	}

	if opts.jsonOut {
		if status != nil {
			b, _ := json.Marshal(statusAsJSON(c, status))
			fmt.Println(string(b))
			return
		}
		fmt.Printf("{\"fallback\":%q}\n", statusMsg)
		return
	}

	line2 := formatStatuslineLine(status, statusMsg, opts.noColor)
	if sess != nil && !opts.jsonOut && !opts.line2Only {
		if line1 := formatStatuslineLine1(sess, opts.noColor); line1 != "" {
			fmt.Println(line1)
		}
	}
	fmt.Println(line2)
}

// loadStatuslineStatus opens the DB lazily via OpenIfExists (§4.9): if the DB
// file is absent, returns ("", "idle") immediately — no cold-create cost. When
// resolution or lookup fails, returns a fallback message.
func loadStatuslineStatus(c *client.Client, wanted string, sess *Session) (*store.Status, string) {
	path, err := datadir.DBPath()
	if err != nil {
		return nil, "offline"
	}
	db, err := store.OpenIfExists(path)
	if err != nil {
		return nil, "offline"
	}
	if db == nil {
		return nil, "idle"
	}
	defer db.Close()

	ctx := context.Background()
	id, err := resolveStatuslineSession(ctx, db, c, wanted, sess)
	if err != nil {
		if strings.Contains(err.Error(), "no sessions") {
			return nil, "idle"
		}
		return nil, "offline"
	}
	s, err := db.StatusGet(ctx, id)
	if err != nil {
		return nil, "offline"
	}
	if s == nil {
		return nil, "idle"
	}
	return s, ""
}

func resolveStatuslineSession(ctx context.Context, db *store.DB, c *client.Client, wanted string, sess *Session) (store.Identity, error) {
	// Priority 1: explicit --session= flag
	if wanted != "" {
		return store.Identity{Host: c.Host(), ProjectDir: c.ProjectDir(), SessionID: wanted}, nil
	}
	// Priority 2: session_id from stdin JSON (exact own-session lookup)
	if sess != nil && sess.SessionID != "" {
		projectDir := c.ProjectDir()
		if sess.Workspace != nil && sess.Workspace.ProjectDir != "" {
			projectDir = sess.Workspace.ProjectDir
		}
		return store.Identity{Host: c.Host(), ProjectDir: projectDir, SessionID: sess.SessionID}, nil
	}
	// Priority 3: global most-recent (fallback for manual invocation)
	sessions, err := db.SessionList(ctx, "", "", "", 1)
	if err != nil {
		return store.Identity{}, err
	}
	if len(sessions) == 0 {
		return store.Identity{}, fmt.Errorf("no sessions")
	}
	s := sessions[0]
	return store.Identity{Host: s.Host, ProjectDir: s.ProjectDir, SessionID: s.SessionID}, nil
}

// formatStatuslineLine renders the single steop line.
// Shape:  steop: [<mode>] <phase> <step>  loop=N tools=N retries=N
// On fallback:  steop: <msg>  (msg ∈ {"idle", "offline", "error"})
func formatStatuslineLine(s *store.Status, fallback string, noColor bool) string {
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

// statusAsJSON shapes a store.Status for CLI --json output.
func statusAsJSON(c *client.Client, s *store.Status) interface{} {
	return map[string]interface{}{
		"id":             client.ComposeSessionID(s.Host, s.ProjectDir, s.SessionID),
		"mode":           s.Mode,
		"phase":          s.Phase,
		"step":           s.Step,
		"tool_calls":     s.ToolCalls,
		"loop_count":     s.LoopCount,
		"step_retry":     s.StepRetry,
		"last_active_at": s.LastActiveAt,
	}
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
