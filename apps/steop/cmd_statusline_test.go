package main

import (
	"os"
	"path/filepath"
	"strings"
	"testing"

	"github.com/tasanakorn/stele/apps/steop/internal/client"
)

func fileExists(path string) bool {
	_, err := os.Stat(path)
	return err == nil
}

// TestStatuslineColdOpen verifies the §4.9 cold-start path: when no DB file
// exists on disk, loadStatuslineStatus must return (nil, "idle") immediately
// without erroring or creating the file.
func TestStatuslineColdOpen(t *testing.T) {
	tmp := t.TempDir()
	dbPath := filepath.Join(tmp, "steop.db")
	t.Setenv("STEOP_DB", dbPath)

	c := client.New("http://127.0.0.1:0", "")
	c = c.WithRequestContext("h", "/p")

	status, msg := loadStatuslineStatus(c, "", nil)
	if status != nil {
		t.Errorf("status = %+v, want nil", status)
	}
	if msg != "idle" {
		t.Errorf("msg = %q, want idle", msg)
	}

	// The DB file must NOT have been created by the statusline path.
	if fileExists(dbPath) {
		t.Errorf("DB file was created by statusline cold-open at %s", dbPath)
	}
}

// TestStatuslineFormatFallback verifies the fallback rendering path.
func TestStatuslineFormatFallback(t *testing.T) {
	got := formatStatuslineLine(nil, "idle", true)
	if !strings.Contains(got, "idle") {
		t.Errorf("got %q, want fallback containing 'idle'", got)
	}
}
