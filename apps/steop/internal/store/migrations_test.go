package store

import (
	"database/sql"
	"path/filepath"
	"testing"
)

// setMigrationsForTest swaps the package-level migrations slice for the
// duration of a test. Used to simulate appending a new migration.
func setMigrationsForTest(t *testing.T, m []func(*sql.Tx) error) {
	t.Helper()
	prev := migrations
	migrations = m
	t.Cleanup(func() { migrations = prev })
}

func TestMigrate_Fresh(t *testing.T) {
	path := filepath.Join(t.TempDir(), "steop.db")
	db, err := Open(path)
	if err != nil {
		t.Fatalf("Open: %v", err)
	}
	defer db.Close()

	var v int
	if err := db.sql.QueryRow("PRAGMA user_version").Scan(&v); err != nil {
		t.Fatalf("user_version: %v", err)
	}
	if v != 1 {
		t.Fatalf("user_version = %d, want 1", v)
	}

	tables := []string{
		"steop_sessions",
		"steop_storage_session",
		"steop_storage_project",
		"steop_logs",
	}
	for _, tbl := range tables {
		var name string
		err := db.sql.QueryRow(
			`SELECT name FROM sqlite_master WHERE type='table' AND name=?`, tbl,
		).Scan(&name)
		if err != nil {
			t.Errorf("table %q missing: %v", tbl, err)
		}
	}
	indexes := []string{
		"idx_steop_sessions_project",
		"idx_steop_logs_session",
		"idx_steop_logs_project",
	}
	for _, idx := range indexes {
		var name string
		err := db.sql.QueryRow(
			`SELECT name FROM sqlite_master WHERE type='index' AND name=?`, idx,
		).Scan(&name)
		if err != nil {
			t.Errorf("index %q missing: %v", idx, err)
		}
	}
}

func TestMigrate_Idempotent(t *testing.T) {
	path := filepath.Join(t.TempDir(), "steop.db")
	db, err := Open(path)
	if err != nil {
		t.Fatalf("Open: %v", err)
	}
	db.Close()

	db2, err := Open(path)
	if err != nil {
		t.Fatalf("Open re-open: %v", err)
	}
	defer db2.Close()
	var v int
	if err := db2.sql.QueryRow("PRAGMA user_version").Scan(&v); err != nil {
		t.Fatalf("user_version: %v", err)
	}
	if v != 1 {
		t.Fatalf("user_version = %d, want 1", v)
	}
}

func TestMigrate_ForwardMigration(t *testing.T) {
	path := filepath.Join(t.TempDir(), "steop.db")
	db, err := Open(path)
	if err != nil {
		t.Fatalf("Open: %v", err)
	}
	db.Close()

	// Append a sentinel migration that leaves a detectable artefact.
	extended := append([]func(*sql.Tx) error{}, migrations...)
	extended = append(extended, func(tx *sql.Tx) error {
		_, err := tx.Exec(`CREATE TABLE steop_test_sentinel (v INTEGER)`)
		if err != nil {
			return err
		}
		_, err = tx.Exec(`INSERT INTO steop_test_sentinel (v) VALUES (42)`)
		return err
	})
	setMigrationsForTest(t, extended)

	db2, err := Open(path)
	if err != nil {
		t.Fatalf("Open after extend: %v", err)
	}
	defer db2.Close()

	var v int
	if err := db2.sql.QueryRow("PRAGMA user_version").Scan(&v); err != nil {
		t.Fatalf("user_version: %v", err)
	}
	if v != 2 {
		t.Fatalf("user_version = %d, want 2", v)
	}
	var got int
	if err := db2.sql.QueryRow(`SELECT v FROM steop_test_sentinel`).Scan(&got); err != nil {
		t.Fatalf("sentinel query: %v", err)
	}
	if got != 42 {
		t.Fatalf("sentinel = %d, want 42", got)
	}
}

func TestMigrate_DowngradeGuard(t *testing.T) {
	path := filepath.Join(t.TempDir(), "steop.db")
	db, err := Open(path)
	if err != nil {
		t.Fatalf("Open: %v", err)
	}
	if _, err := db.sql.Exec("PRAGMA user_version = 99"); err != nil {
		t.Fatalf("set user_version: %v", err)
	}
	db.Close()

	if _, err := Open(path); err == nil {
		t.Fatal("expected ErrSchemaNewer")
	} else if err != ErrSchemaNewer {
		// Error is wrapped through fmt.Errorf? We return errSchemaNewer as-is.
		t.Fatalf("got %v, want ErrSchemaNewer", err)
	}
}
