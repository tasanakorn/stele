package main

import (
	"bytes"
	"context"
	"encoding/json"
	"fmt"
	"io"
	"math"
	"os"
	"os/exec"
	"path/filepath"
	"strings"
	"time"
)

// Session types for line 1 rendering. Pointer fields for numerics so "absent"
// is distinguishable from "zero".

type Session struct {
	SessionID     string                `json:"session_id,omitempty"`
	Model         *SessionModel         `json:"model,omitempty"`
	Workspace     *SessionWorkspace     `json:"workspace,omitempty"`
	ContextWindow *SessionContextWindow `json:"context_window,omitempty"`
	RateLimits    *SessionRateLimits    `json:"rate_limits,omitempty"`
	Cost          *SessionCost          `json:"cost,omitempty"`
}

type SessionModel struct {
	ID          string `json:"id,omitempty"`
	DisplayName string `json:"display_name,omitempty"`
}

type SessionWorkspace struct {
	ProjectDir string `json:"project_dir,omitempty"`
}

type SessionContextWindow struct {
	UsedPercentage    *float64 `json:"used_percentage,omitempty"`
	ContextWindowSize *int64   `json:"context_window_size,omitempty"`
}

type SessionRateLimits struct {
	FiveHour *SessionRateWindow `json:"five_hour,omitempty"`
	SevenDay *SessionRateWindow `json:"seven_day,omitempty"`
}

type SessionRateWindow struct {
	UsedPercentage *float64 `json:"used_percentage,omitempty"`
	ResetsAt       *int64   `json:"resets_at,omitempty"`
}

type SessionCost struct {
	TotalCostUSD *float64 `json:"total_cost_usd,omitempty"`
}

// parseSession reads up to 1 MiB from r and unmarshals a Session. Returns
// (nil, false) on empty input or parse error.
func parseSession(r io.Reader) (*Session, bool) {
	b, err := io.ReadAll(io.LimitReader(r, 1<<20))
	if err != nil || len(bytes.TrimSpace(b)) == 0 {
		return nil, false
	}
	var sess Session
	if err := json.Unmarshal(b, &sess); err != nil {
		return nil, false
	}
	return &sess, true
}

// formatStatuslineLine1 builds the line-1 statusline string from session JSON.
// Returns "" when no segments are present (caller should skip output).
func formatStatuslineLine1(s *Session, noColor bool) string {
	var parts []string

	// Segment: Model
	if s.Model != nil {
		name := s.Model.DisplayName
		if name == "" {
			name = s.Model.ID
		}
		if name != "" {
			parts = append(parts, wrap(name, statuslineBrightCyan, noColor))
		}
	}

	// Segment: Project directory
	projectDir := ""
	if s.Workspace != nil && s.Workspace.ProjectDir != "" {
		projectDir = s.Workspace.ProjectDir
		parts = append(parts, wrap(filepath.Base(projectDir), statuslineBrightWhite, noColor))
	}

	// Segment: Git branch
	if branch := segBranch(projectDir); branch != "" {
		parts = append(parts, wrap(branch, statuslineBrightGreen, noColor))
	}

	// Segment: Context window bar
	parts = append(parts, segContextBar(s.ContextWindow, noColor))

	// Segment: Cost / rate limits
	if seg := segRateCost(s, noColor); seg != "" {
		parts = append(parts, seg)
	}

	return joinLine1Segments(parts, noColor)
}

// segBranch runs `git branch --show-current` in projectDir (or cwd on error)
// with a 500 ms timeout. Returns "" on any error or empty output.
func segBranch(projectDir string) string {
	ctx, cancel := context.WithTimeout(context.Background(), 500*time.Millisecond)
	defer cancel()

	cmd := exec.CommandContext(ctx, "git", "branch", "--show-current")
	if projectDir != "" {
		cmd.Dir = projectDir
	} else {
		if cwd, err := os.Getwd(); err == nil {
			cmd.Dir = cwd
		}
	}
	cmd.Stderr = nil

	out, err := cmd.Output()
	if err != nil {
		return ""
	}
	return strings.TrimRight(string(out), "\n")
}

// segContextBar builds the 8-wide progress bar + percentage segment.
func segContextBar(cw *SessionContextWindow, noColor bool) string {
	var pct float64
	if cw != nil && cw.UsedPercentage != nil {
		pct = *cw.UsedPercentage
	}

	const width = 8
	filled := int(math.Round(pct / 100 * width))
	if filled < 0 {
		filled = 0
	}
	if filled > width {
		filled = width
	}
	bar := strings.Repeat("█", filled) + strings.Repeat("░", width-filled)

	barColor := statuslineBrightGreen
	if cw != nil && cw.ContextWindowSize != nil && *cw.ContextWindowSize > 0 {
		size := *cw.ContextWindowSize
		used := int64(math.Round(pct / 100 * float64(size)))
		hard := int64(math.Round(0.8 * float64(size)))
		if used >= hard {
			barColor = statuslineBrightRed
		} else if used >= 160000 {
			barColor = statuslineBrightYellow
		}
	} else {
		pctInt := int(math.Round(pct))
		if pctInt >= 85 {
			barColor = statuslineBrightRed
		} else if pctInt >= 60 {
			barColor = statuslineBrightYellow
		}
	}

	pctInt := int(math.Round(pct))
	return wrap(bar, barColor, noColor) + " " + wrap(fmt.Sprintf("%d%%", pctInt), statuslineBrightWhite, noColor)
}

// segRateCost builds the rate-limit or cost segment. Returns "" when neither
// is available.
func segRateCost(s *Session, noColor bool) string {
	if s.RateLimits != nil && s.RateLimits.FiveHour != nil &&
		s.RateLimits.FiveHour.UsedPercentage != nil {
		return segRateLimits(s.RateLimits, noColor)
	}
	if s.Cost != nil && s.Cost.TotalCostUSD != nil && *s.Cost.TotalCostUSD != 0 {
		return wrap(fmt.Sprintf("$%.2f", *s.Cost.TotalCostUSD), statuslineBrightYellow, noColor)
	}
	return ""
}

// segRateLimits builds the 5h/7d rate-limit segment.
func segRateLimits(rl *SessionRateLimits, noColor bool) string {
	var segs []string

	if rl.FiveHour != nil && rl.FiveHour.UsedPercentage != nil {
		segs = append(segs, segRateWindow("5h", rl.FiveHour, 18000, noColor))
	}
	if rl.SevenDay != nil && rl.SevenDay.UsedPercentage != nil {
		segs = append(segs, segRateWindow("7d", rl.SevenDay, 604800, noColor))
	}

	return strings.Join(segs, " ")
}

// segRateWindow renders a single rate-limit window (e.g. "5h:35%/22%").
func segRateWindow(label string, w *SessionRateWindow, windowSec int64, noColor bool) string {
	rateInt := int(math.Round(*w.UsedPercentage))
	labelPart := wrap(label+":", statuslineBrightCyan, noColor)

	if w.ResetsAt != nil {
		elapsed := elapsedPct(*w.ResetsAt, windowSec)
		var valueColor string
		if rateInt > elapsed {
			valueColor = statuslineBrightYellow
		} else {
			valueColor = statuslineBrightGreen
		}
		return labelPart + wrap(fmt.Sprintf("%d%%/%d%%", rateInt, elapsed), valueColor, noColor)
	}
	// No resets_at — fall back to plain percent, no color
	return labelPart + wrap(fmt.Sprintf("%d%%", rateInt), statuslineBrightWhite, noColor)
}

// elapsedPct computes the integer percentage of the window that has elapsed,
// given resetsAt (Unix epoch when the window next resets) and window length in
// seconds. Clamps to [0, 100].
func elapsedPct(resetsAt int64, window int64) int {
	now := time.Now().Unix()
	start := resetsAt - window
	elapsed := now - start
	if elapsed < 0 {
		elapsed = 0
	}
	if elapsed > window {
		elapsed = window
	}
	return int(math.Round(float64(elapsed) * 100 / float64(window)))
}

// joinLine1Segments joins non-empty parts with a muted separator.
func joinLine1Segments(parts []string, noColor bool) string {
	sep := "\x1b[37m | \x1b[0m"
	if noColor {
		sep = " | "
	}
	return strings.Join(parts, sep)
}

// wrap applies an ANSI color code around text, or returns text unchanged when
// noColor is true or color is empty.
func wrap(text, color string, noColor bool) string {
	if noColor || color == "" {
		return text
	}
	return color + text + statuslineReset
}
