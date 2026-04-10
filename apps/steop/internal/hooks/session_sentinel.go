package hooks

import (
	"os"
	"path/filepath"
	"strings"
)

// SentinelPath returns ~/.config/stele/steop-current-session.
// Returns empty string if the home directory cannot be resolved.
func SentinelPath() string {
	home, err := os.UserHomeDir()
	if err != nil || home == "" {
		return ""
	}
	return filepath.Join(home, ".config", "stele", "steop-current-session")
}

// WriteSentinel writes sessionID to the sentinel file, creating parent dirs.
// Idempotent: if the file already contains sessionID, it is not rewritten.
func WriteSentinel(sessionID string) error {
	path := SentinelPath()
	if path == "" {
		return nil
	}
	if existing := ReadSentinel(); existing == sessionID {
		return nil
	}
	dir := filepath.Dir(path)
	if err := os.MkdirAll(dir, 0o755); err != nil {
		return err
	}
	return os.WriteFile(path, []byte(sessionID+"\n"), 0o644)
}

// ReadSentinel returns the session id stored in the sentinel file,
// or "" if the file is missing / unreadable / empty.
func ReadSentinel() string {
	path := SentinelPath()
	if path == "" {
		return ""
	}
	data, err := os.ReadFile(path)
	if err != nil {
		return ""
	}
	return strings.TrimSpace(string(data))
}
