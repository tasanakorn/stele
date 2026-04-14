package datadir

import (
	"os"
	"path/filepath"
	"strings"
	"testing"
)

func TestDBPath_StepEnvOverride(t *testing.T) {
	dir := t.TempDir()
	custom := filepath.Join(dir, "nested", "custom.db")
	t.Setenv("STEOP_DB", custom)
	t.Setenv("XDG_DATA_HOME", filepath.Join(dir, "xdg"))
	t.Setenv("HOME", filepath.Join(dir, "home"))

	got, err := DBPath()
	if err != nil {
		t.Fatalf("DBPath: %v", err)
	}
	if got != custom {
		t.Fatalf("want %q, got %q", custom, got)
	}
	info, err := os.Stat(filepath.Dir(custom))
	if err != nil {
		t.Fatalf("parent dir missing: %v", err)
	}
	if !info.IsDir() {
		t.Fatal("parent is not a dir")
	}
}

func TestDBPath_XDGDataHome(t *testing.T) {
	dir := t.TempDir()
	xdg := filepath.Join(dir, "xdg")
	t.Setenv("STEOP_DB", "")
	t.Setenv("XDG_DATA_HOME", xdg)
	t.Setenv("HOME", filepath.Join(dir, "home"))

	got, err := DBPath()
	if err != nil {
		t.Fatalf("DBPath: %v", err)
	}
	want := filepath.Join(xdg, "steop", "steop.db")
	if got != want {
		t.Fatalf("want %q, got %q", want, got)
	}
	if _, err := os.Stat(filepath.Dir(got)); err != nil {
		t.Fatalf("parent dir missing: %v", err)
	}
}

func TestDBPath_HomeFallback(t *testing.T) {
	dir := t.TempDir()
	home := filepath.Join(dir, "home")
	t.Setenv("STEOP_DB", "")
	t.Setenv("XDG_DATA_HOME", "")
	t.Setenv("HOME", home)

	got, err := DBPath()
	if err != nil {
		t.Fatalf("DBPath: %v", err)
	}
	want := filepath.Join(home, ".local", "share", "steop", "steop.db")
	if got != want {
		t.Fatalf("want %q, got %q", want, got)
	}
	if !strings.HasSuffix(got, filepath.Join(".local", "share", "steop", "steop.db")) {
		t.Fatalf("unexpected path shape: %q", got)
	}
	if _, err := os.Stat(filepath.Dir(got)); err != nil {
		t.Fatalf("parent dir missing: %v", err)
	}
}

func TestDBPath_NoHome(t *testing.T) {
	t.Setenv("STEOP_DB", "")
	t.Setenv("XDG_DATA_HOME", "")
	t.Setenv("HOME", "")
	if _, err := DBPath(); err == nil {
		t.Fatal("expected error when HOME is unset")
	}
}
