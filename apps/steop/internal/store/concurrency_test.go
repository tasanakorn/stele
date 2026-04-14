package store

import (
	"context"
	"encoding/json"
	"path/filepath"
	"sync"
	"testing"
)

// TestConcurrentStateIncr spawns N=32 goroutines each doing a StateIncr on
// the same session against one on-disk DB. Asserts the final counter is 32
// with no errors — exercises the BEGIN IMMEDIATE + busy_timeout loop that
// the hook hot path relies on.
func TestConcurrentStateIncr(t *testing.T) {
	path := filepath.Join(t.TempDir(), "steop.db")
	db, err := Open(path)
	if err != nil {
		t.Fatalf("Open: %v", err)
	}
	defer db.Close()

	ctx := context.Background()
	id := mustIdent(t, "h:/p:a1b2c3d4-5678-4abc-9def-0123456789ab")
	if _, err := db.SessionStart(ctx, id, nil); err != nil {
		t.Fatalf("SessionStart: %v", err)
	}

	const N = 32
	var wg sync.WaitGroup
	errCh := make(chan error, N)
	for i := 0; i < N; i++ {
		wg.Add(1)
		go func() {
			defer wg.Done()
			if _, err := db.StateIncr(ctx, id, "tool_calls", 1); err != nil {
				errCh <- err
			}
		}()
	}
	wg.Wait()
	close(errCh)
	for err := range errCh {
		t.Errorf("incr error: %v", err)
	}

	sess, err := db.SessionGet(ctx, id)
	if err != nil {
		t.Fatalf("SessionGet: %v", err)
	}
	var counters map[string]int64
	if err := json.Unmarshal(sess.Counters, &counters); err != nil {
		t.Fatalf("decode counters: %v", err)
	}
	if counters["tool_calls"] != N {
		t.Fatalf("tool_calls = %d, want %d", counters["tool_calls"], N)
	}
}
