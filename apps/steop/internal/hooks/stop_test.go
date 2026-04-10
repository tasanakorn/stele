package hooks

import (
	"strings"
	"testing"
)

func TestBuildBodyEmpty(t *testing.T) {
	if got := buildBody(""); got != "Session finished" {
		t.Errorf("got %q", got)
	}
}

func TestBuildBodyCollapsesWhitespace(t *testing.T) {
	if got := buildBody("  hello\nworld  "); got != "hello world" {
		t.Errorf("got %q", got)
	}
}

func TestBuildBodyTruncates(t *testing.T) {
	in := strings.Repeat("a", 200)
	got := buildBody(in)
	if !strings.HasSuffix(got, "…") {
		t.Errorf("expected ellipsis suffix, got %q", got)
	}
	if n := len([]rune(got)); n != maxBodyLen {
		t.Errorf("expected %d runes, got %d", maxBodyLen, n)
	}
}

func TestDefaultTitleWithCwd(t *testing.T) {
	if got := defaultTitle("/tmp/project"); got != "Claude Code · project" {
		t.Errorf("got %q", got)
	}
}

func TestDefaultTitleEmpty(t *testing.T) {
	if got := defaultTitle(""); got != "Claude Code" {
		t.Errorf("got %q", got)
	}
}
