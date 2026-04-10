package main

import (
	"strings"
	"testing"
	"time"
)

func ptr[T any](v T) *T { return &v }

// formatStatuslineLine1 tests — noColor=true so output is deterministic.

func TestFormatStatuslineLine1_Empty(t *testing.T) {
	s := &Session{}
	got := formatStatuslineLine1(s, true)
	// No model, no workspace → only the context bar (0%) is emitted;
	// result will just be the bar segment, not empty string. But per spec:
	// "Empty segments mean line 1 omitted — caller should skip."
	// The context bar is always rendered, so the result is non-empty.
	// Verify it at least contains a bar character and "0%".
	if !strings.Contains(got, "0%") {
		t.Errorf("expected 0%% in output for empty session, got: %q", got)
	}
}

func TestFormatStatuslineLine1_ModelDisplayName(t *testing.T) {
	s := &Session{
		Model: &SessionModel{DisplayName: "claude-sonnet-4-5"},
	}
	got := formatStatuslineLine1(s, true)
	if !strings.Contains(got, "claude-sonnet-4-5") {
		t.Errorf("expected model display_name in output, got: %q", got)
	}
}

func TestFormatStatuslineLine1_ModelFallbackToID(t *testing.T) {
	s := &Session{
		Model: &SessionModel{ID: "claude-opus-4"},
	}
	got := formatStatuslineLine1(s, true)
	if !strings.Contains(got, "claude-opus-4") {
		t.Errorf("expected model ID fallback in output, got: %q", got)
	}
}

func TestFormatStatuslineLine1_ModelAndProject(t *testing.T) {
	s := &Session{
		Model:     &SessionModel{DisplayName: "claude-sonnet-4-5"},
		Workspace: &SessionWorkspace{ProjectDir: "/home/user/stele"},
	}
	got := formatStatuslineLine1(s, true)
	if !strings.Contains(got, "claude-sonnet-4-5") {
		t.Errorf("expected model name, got: %q", got)
	}
	if !strings.Contains(got, "stele") {
		t.Errorf("expected project basename 'stele', got: %q", got)
	}
}

func TestFormatStatuslineLine1_ContextBar50Pct200K(t *testing.T) {
	s := &Session{
		ContextWindow: &SessionContextWindow{
			UsedPercentage:    ptr(50.0),
			ContextWindowSize: ptr(int64(200000)),
		},
	}
	got := formatStatuslineLine1(s, true)
	if !strings.Contains(got, "50%") {
		t.Errorf("expected '50%%' in context bar output, got: %q", got)
	}
	// At 50% with 200K window: used=100K, hard=160K → green, bar has 4 filled
	if !strings.Contains(got, "████") {
		t.Errorf("expected at least 4 filled bar blocks, got: %q", got)
	}
}

func TestFormatStatuslineLine1_ContextBar85Pct200K(t *testing.T) {
	// 85% × 200K = 170K > 160K (hard=160K) → red
	s := &Session{
		ContextWindow: &SessionContextWindow{
			UsedPercentage:    ptr(85.0),
			ContextWindowSize: ptr(int64(200000)),
		},
	}
	got := formatStatuslineLine1(s, true)
	if !strings.Contains(got, "85%") {
		t.Errorf("expected '85%%' substring, got: %q", got)
	}
}

func TestFormatStatuslineLine1_ContextBar20Pct1M(t *testing.T) {
	// 20% × 1M = 200K > 160K → yellow (soft limit)
	s := &Session{
		ContextWindow: &SessionContextWindow{
			UsedPercentage:    ptr(20.0),
			ContextWindowSize: ptr(int64(1000000)),
		},
	}
	got := formatStatuslineLine1(s, true)
	if !strings.Contains(got, "20%") {
		t.Errorf("expected '20%%' substring, got: %q", got)
	}
	if !strings.Contains(got, "░") || !strings.Contains(got, "█") {
		// bar must be present
		t.Errorf("expected bar characters in output, got: %q", got)
	}
}

func TestFormatStatuslineLine1_ContextBarNoWindowSize(t *testing.T) {
	// Fallback path: no context_window_size, percent<60 → green
	s := &Session{
		ContextWindow: &SessionContextWindow{
			UsedPercentage: ptr(40.0),
		},
	}
	got := formatStatuslineLine1(s, true)
	if !strings.Contains(got, "40%") {
		t.Errorf("expected '40%%' in output, got: %q", got)
	}
}

func TestFormatStatuslineLine1_RateLimits5hWithResetsAt(t *testing.T) {
	// elapsed = 25% of 18000s window: resetsAt = now + 13500
	resetsAt := time.Now().Unix() + 13500
	s := &Session{
		RateLimits: &SessionRateLimits{
			FiveHour: &SessionRateWindow{
				UsedPercentage: ptr(50.0),
				ResetsAt:       &resetsAt,
			},
		},
	}
	got := formatStatuslineLine1(s, true)
	if !strings.Contains(got, "5h:") {
		t.Errorf("expected '5h:' label, got: %q", got)
	}
	if !strings.Contains(got, "50%") {
		t.Errorf("expected '50%%' in output, got: %q", got)
	}
	// elapsed ≈ 25%; used 50% > elapsed 25% → yellow but we only check substring
	if !strings.Contains(got, "/") {
		t.Errorf("expected used%%/elapsed%% format, got: %q", got)
	}
}

func TestFormatStatuslineLine1_RateLimits5hMissingResetsAt(t *testing.T) {
	s := &Session{
		RateLimits: &SessionRateLimits{
			FiveHour: &SessionRateWindow{
				UsedPercentage: ptr(50.0),
			},
		},
	}
	got := formatStatuslineLine1(s, true)
	if !strings.Contains(got, "5h:50%") {
		t.Errorf("expected '5h:50%%' (no slash), got: %q", got)
	}
	if strings.Contains(got, "/") {
		t.Errorf("expected no slash when resets_at missing, got: %q", got)
	}
}

func TestFormatStatuslineLine1_CostOnly(t *testing.T) {
	s := &Session{
		Cost: &SessionCost{TotalCostUSD: ptr(1.234)},
	}
	got := formatStatuslineLine1(s, true)
	if !strings.Contains(got, "$1.23") {
		t.Errorf("expected '$1.23' in output, got: %q", got)
	}
}

func TestFormatStatuslineLine1_RateLimitsBeatsCost(t *testing.T) {
	resetsAt := time.Now().Unix() + 9000
	s := &Session{
		RateLimits: &SessionRateLimits{
			FiveHour: &SessionRateWindow{
				UsedPercentage: ptr(40.0),
				ResetsAt:       &resetsAt,
			},
		},
		Cost: &SessionCost{TotalCostUSD: ptr(9.99)},
	}
	got := formatStatuslineLine1(s, true)
	if !strings.Contains(got, "5h:") {
		t.Errorf("expected rate limits to win over cost, got: %q", got)
	}
	if strings.Contains(got, "$9.99") {
		t.Errorf("cost should be suppressed when rate limits present, got: %q", got)
	}
}

// parseSession tests

func TestParseSession_Empty(t *testing.T) {
	s, ok := parseSession(strings.NewReader(""))
	if ok || s != nil {
		t.Errorf("expected (nil, false) for empty input, got (%v, %v)", s, ok)
	}
}

func TestParseSession_EmptyObject(t *testing.T) {
	s, ok := parseSession(strings.NewReader("{}"))
	if !ok || s == nil {
		t.Errorf("expected success for '{}', got (%v, %v)", s, ok)
	}
	if s.Model != nil || s.Workspace != nil || s.Cost != nil {
		t.Errorf("expected nil sub-fields for '{}', got %+v", s)
	}
}

func TestParseSession_Malformed(t *testing.T) {
	s, ok := parseSession(strings.NewReader("{not valid json"))
	if ok || s != nil {
		t.Errorf("expected (nil, false) for malformed JSON, got (%v, %v)", s, ok)
	}
}
