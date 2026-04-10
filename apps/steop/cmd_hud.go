package main

import (
	"encoding/json"
	"errors"
	"fmt"
	"os"
	"os/signal"
	"strconv"
	"strings"
	"syscall"
	"time"

	"github.com/tasanakorn/stele/apps/steop/internal/client"
)

const (
	ansiClearEOL   = "\x1b[K"
	ansiCursorHome = "\x1b[H"
	ansiHideCursor = "\x1b[?25l"
	ansiShowCursor = "\x1b[?25h"
	ansiReset      = "\x1b[0m"
	ansiBold       = "\x1b[1m"
	ansiDim        = "\x1b[2m"
)

var phaseColors = map[string]string{
	"clarify":  "\x1b[36m",
	"research": "\x1b[34m",
	"plan":     "\x1b[32m",
	"execute":  "\x1b[33m",
	"validate": "\x1b[35m",
}

type hudOpts struct {
	session  string
	once     bool
	jsonOut  bool
	interval time.Duration
	noColor  bool
}

func runHud(args []string) {
	opts := hudOpts{interval: 1 * time.Second}
	for _, a := range args {
		switch {
		case a == "-h" || a == "--help":
			printHudUsage()
			return
		case a == "--once":
			opts.once = true
		case a == "--json":
			opts.jsonOut = true
		case a == "--no-color":
			opts.noColor = true
		case strings.HasPrefix(a, "--session="):
			opts.session = a[len("--session="):]
		case strings.HasPrefix(a, "--interval="):
			v, err := strconv.ParseFloat(a[len("--interval="):], 64)
			if err != nil || v <= 0 {
				fmt.Fprintf(os.Stderr, "hud: invalid --interval value: %q\n", a[len("--interval="):])
				os.Exit(2)
			}
			opts.interval = time.Duration(v * float64(time.Second))
		default:
			fmt.Fprintf(os.Stderr, "hud: unknown argument: %q\n", a)
			printHudUsage()
			os.Exit(2)
		}
	}

	if os.Getenv("NO_COLOR") != "" {
		opts.noColor = true
	}

	c, err := client.NewFromConfig()
	if err != nil {
		fmt.Fprintf(os.Stderr, "hud: client init: %v\n", err)
		os.Exit(1)
	}

	if opts.once {
		runHudOnce(c, opts)
		return
	}
	runHudLoop(c, opts)
}

func runHudOnce(c *client.Client, opts hudOpts) {
	sid, err := resolveSessionID(c, opts.session)
	if err != nil {
		fmt.Fprintf(os.Stderr, "hud: %v\n", err)
		os.Exit(2)
	}
	status, err := c.StatusGet(sid)
	if err != nil {
		if errors.Is(err, client.ErrNotFound) {
			fmt.Fprintf(os.Stderr, "hud: session not found: %s\n", sid)
			os.Exit(2)
		}
		fmt.Fprintf(os.Stderr, "hud: status: %v\n", err)
		os.Exit(1)
	}
	if opts.jsonOut {
		b, _ := json.Marshal(status)
		fmt.Println(string(b))
		return
	}
	fmt.Println(formatHudLine(status, opts.noColor))
}

func runHudLoop(c *client.Client, opts hudOpts) {
	sigCh := make(chan os.Signal, 1)
	signal.Notify(sigCh, syscall.SIGINT, syscall.SIGTERM)

	ticker := time.NewTicker(opts.interval)
	defer ticker.Stop()

	firstDraw := true
	if !opts.jsonOut && !opts.noColor {
		fmt.Print(ansiHideCursor)
	}
	cleanup := func() {
		if !opts.jsonOut && !opts.noColor {
			fmt.Print(ansiShowCursor)
		}
		fmt.Println()
	}

	draw := func() {
		sid, err := resolveSessionID(c, opts.session)
		if err != nil {
			renderWaiting(firstDraw, opts, err.Error())
			firstDraw = false
			return
		}
		status, err := c.StatusGet(sid)
		if err != nil {
			if errors.Is(err, client.ErrNotFound) {
				renderWaiting(firstDraw, opts, "waiting for session "+sid+"...")
				firstDraw = false
				return
			}
			renderWaiting(firstDraw, opts, "error: "+err.Error())
			firstDraw = false
			return
		}
		if opts.jsonOut {
			b, _ := json.Marshal(status)
			fmt.Println(string(b))
			return
		}
		renderPanel(status, firstDraw, opts)
		firstDraw = false
	}

	draw()
	for {
		select {
		case <-sigCh:
			cleanup()
			return
		case <-ticker.C:
			draw()
		}
	}
}

func resolveSessionID(c *client.Client, wanted string) (string, error) {
	if wanted != "" {
		return wanted, nil
	}
	sessions, err := c.SessionsList(1)
	if err != nil {
		return "", fmt.Errorf("list sessions: %w", err)
	}
	if len(sessions) == 0 {
		return "", fmt.Errorf("no sessions found (run a st-flow task first)")
	}
	return sessions[0].SessionID, nil
}

func renderPanel(s *client.HudStatus, firstDraw bool, opts hudOpts) {
	if !firstDraw {
		fmt.Print(ansiCursorHome)
	}
	color := ""
	reset := ""
	bold := ""
	dim := ""
	if !opts.noColor {
		if c, ok := phaseColors[strings.ToLower(s.Phase)]; ok {
			color = c
		}
		reset = ansiReset
		bold = ansiBold
		dim = ansiDim
	}
	phase := s.Phase
	if phase == "" {
		phase = "-"
	}
	mode := s.Mode
	if mode == "" {
		mode = "-"
	}
	step := s.Step
	if step == "" {
		step = "-"
	}
	updated := s.UpdatedAt
	if len(updated) > 19 {
		updated = updated[:19]
	}
	sid := s.SessionID
	if len(sid) > 36 {
		sid = sid[:36]
	}

	line := func(label, value string) {
		fmt.Printf("%s%-10s%s %s%s\n", dim, label, reset, value, ansiClearEOL)
	}

	fmt.Printf("%s%s steop HUD %s%s%s\n", bold, color, sid, reset, ansiClearEOL)
	line("phase", color+phase+reset)
	line("mode", mode)
	line("step", step)
	line("counters", fmt.Sprintf("loop=%d  tools=%d  retries=%d",
		s.LoopCount, s.ToolCalls, s.StepRetry))
	line("updated", updated)
}

func renderWaiting(firstDraw bool, opts hudOpts, msg string) {
	if opts.jsonOut {
		fmt.Printf(`{"waiting":true,"message":%q}`+"\n", msg)
		return
	}
	if !firstDraw {
		fmt.Print(ansiCursorHome)
	}
	bold := ""
	reset := ""
	dim := ""
	if !opts.noColor {
		bold = ansiBold
		reset = ansiReset
		dim = ansiDim
	}
	fmt.Printf("%s steop HUD %s%s\n", bold+reset, "", ansiClearEOL)
	fmt.Printf("%s%s%s%s\n", dim, msg, reset, ansiClearEOL)
	for i := 0; i < 4; i++ {
		fmt.Print(ansiClearEOL + "\n")
	}
}

func formatHudLine(s *client.HudStatus, noColor bool) string {
	phase := s.Phase
	if phase == "" {
		phase = "-"
	}
	step := s.Step
	if step == "" {
		step = "-"
	}
	if noColor {
		return fmt.Sprintf("[%s] %s step=%s loop=%d tools=%d",
			s.Mode, phase, step, s.LoopCount, s.ToolCalls)
	}
	color := phaseColors[strings.ToLower(phase)]
	return fmt.Sprintf("[%s] %s%s%s step=%s loop=%d tools=%d",
		s.Mode, color, phase, ansiReset, step, s.LoopCount, s.ToolCalls)
}

func printHudUsage() {
	fmt.Fprintln(os.Stderr, "usage: steop hud [--session=<id>] [--once] [--json] [--interval=<seconds>] [--no-color]")
	fmt.Fprintln(os.Stderr, "       --session=<id>        session to watch (default: most recently updated)")
	fmt.Fprintln(os.Stderr, "       --once                print once and exit (for tmux status-right, etc.)")
	fmt.Fprintln(os.Stderr, "       --json                emit newline-delimited JSON instead of a panel")
	fmt.Fprintln(os.Stderr, "       --interval=<seconds>  poll interval (default 1; accepts fractions e.g. 0.5)")
	fmt.Fprintln(os.Stderr, "       --no-color            disable ANSI colors")
}
