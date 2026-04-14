package store

import (
	"path/filepath"
	"testing"
)

func TestOpen_PragmasApplied(t *testing.T) {
	path := filepath.Join(t.TempDir(), "steop.db")
	db, err := Open(path)
	if err != nil {
		t.Fatalf("Open: %v", err)
	}
	defer db.Close()

	checks := []struct {
		name string
		want string
	}{
		{"journal_mode", "wal"},
		{"busy_timeout", "5000"},
		{"synchronous", "1"}, // NORMAL == 1
		{"foreign_keys", "1"},
		{"temp_store", "2"}, // MEMORY == 2
	}
	for _, c := range checks {
		var got string
		row := db.sql.QueryRow("PRAGMA " + c.name)
		if err := row.Scan(&got); err != nil {
			t.Fatalf("PRAGMA %s: %v", c.name, err)
		}
		if got != c.want {
			t.Errorf("PRAGMA %s = %q, want %q", c.name, got, c.want)
		}
	}
}
