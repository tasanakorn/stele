package datadir

import (
	"errors"
	"os"
	"path/filepath"
)

// DBPath resolves the steop SQLite path per PRD-020 §4.1.
// Precedence:
//  1. $STEOP_DB (used verbatim)
//  2. $XDG_DATA_HOME/steop/steop.db
//  3. $HOME/.local/share/steop/steop.db
//
// The parent directory is created with mode 0700. The file itself is not
// opened or touched.
func DBPath() (string, error) {
	if v := os.Getenv("STEOP_DB"); v != "" {
		if err := ensureParent(v); err != nil {
			return "", err
		}
		return v, nil
	}
	if v := os.Getenv("XDG_DATA_HOME"); v != "" {
		p := filepath.Join(v, "steop", "steop.db")
		if err := ensureParent(p); err != nil {
			return "", err
		}
		return p, nil
	}
	home := os.Getenv("HOME")
	if home == "" {
		return "", errors.New("HOME is not set; cannot resolve default STEOP_DB path")
	}
	p := filepath.Join(home, ".local", "share", "steop", "steop.db")
	if err := ensureParent(p); err != nil {
		return "", err
	}
	return p, nil
}

func ensureParent(path string) error {
	dir := filepath.Dir(path)
	if dir == "" || dir == "." || dir == "/" {
		return nil
	}
	return os.MkdirAll(dir, 0o700)
}
