package hooks

import (
	"context"
	"encoding/json"
	"path/filepath"
	"testing"

	"github.com/tasanakorn/stele/apps/steop/internal/store"
)

func openTempDB(t *testing.T) *store.DB {
	t.Helper()
	path := filepath.Join(t.TempDir(), "steop.db")
	db, err := store.Open(path)
	if err != nil {
		t.Fatalf("store.Open: %v", err)
	}
	t.Cleanup(func() { db.Close() })
	return db
}

func identFixture(t *testing.T) store.Identity {
	t.Helper()
	id, err := store.ParseID("h:/p:a1b2c3d4-5678-4abc-9def-0123456789ab")
	if err != nil {
		t.Fatalf("ParseID: %v", err)
	}
	return id
}

// TestPostToolUse_Transaction asserts that a single postToolUseTx run lands
// the tool_calls increment, last_tool data merge, and post_tool_use log row
// atomically (PRD-020 §4.5).
func TestPostToolUse_Transaction(t *testing.T) {
	ctx := context.Background()
	db := openTempDB(t)
	id := identFixture(t)

	if err := postToolUseTx(ctx, db, id, "Bash"); err != nil {
		t.Fatalf("postToolUseTx: %v", err)
	}

	sess, err := db.SessionGet(ctx, id)
	if err != nil {
		t.Fatalf("SessionGet: %v", err)
	}
	if sess == nil {
		t.Fatal("session row missing after postToolUseTx")
	}

	var counters map[string]int64
	if err := json.Unmarshal(sess.Counters, &counters); err != nil {
		t.Fatalf("decode counters: %v", err)
	}
	if counters["tool_calls"] != 1 {
		t.Errorf("tool_calls = %d, want 1", counters["tool_calls"])
	}

	var data map[string]any
	if err := json.Unmarshal(sess.Data, &data); err != nil {
		t.Fatalf("decode data: %v", err)
	}
	if data["last_tool"] != "Bash" {
		t.Errorf("last_tool = %v, want Bash", data["last_tool"])
	}
	if _, ok := data["last_tool_at"].(string); !ok {
		t.Errorf("last_tool_at missing from data: %v", data)
	}

	rows, err := db.LogQuery(ctx, id.Host, id.ProjectDir, id.SessionID, 0)
	if err != nil {
		t.Fatalf("LogQuery: %v", err)
	}
	if len(rows) != 1 {
		t.Fatalf("logs = %d, want 1", len(rows))
	}
	if rows[0].Event != "post_tool_use" {
		t.Errorf("event = %q, want post_tool_use", rows[0].Event)
	}
}

// BenchmarkPostToolUse exercises the hook's hot path — one tx per call on a
// fresh DB — so the budget from §8.7 can be checked.
func BenchmarkPostToolUse(b *testing.B) {
	ctx := context.Background()
	path := filepath.Join(b.TempDir(), "steop.db")
	db, err := store.Open(path)
	if err != nil {
		b.Fatalf("Open: %v", err)
	}
	defer db.Close()
	id, err := store.ParseID("h:/p:a1b2c3d4-5678-4abc-9def-0123456789ab")
	if err != nil {
		b.Fatalf("ParseID: %v", err)
	}
	b.ResetTimer()
	for i := 0; i < b.N; i++ {
		if err := postToolUseTx(ctx, db, id, "Bash"); err != nil {
			b.Fatalf("postToolUseTx: %v", err)
		}
	}
}
