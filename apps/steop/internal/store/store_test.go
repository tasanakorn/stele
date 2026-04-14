package store

import (
	"context"
	"encoding/json"
	"path/filepath"
	"testing"
)

func openTestDB(t *testing.T) *DB {
	t.Helper()
	// Use a temp file instead of :memory: so the DSN path of buildDSN exercises
	// the same code path as production. modernc.org/sqlite supports :memory:
	// but the URI needs tweaking; tempfile is simpler and fast.
	path := filepath.Join(t.TempDir(), "steop.db")
	db, err := Open(path)
	if err != nil {
		t.Fatalf("Open: %v", err)
	}
	t.Cleanup(func() { db.Close() })
	return db
}

func mustIdent(t *testing.T, s string) Identity {
	t.Helper()
	id, err := ParseID(s)
	if err != nil {
		t.Fatalf("ParseID(%q): %v", s, err)
	}
	return id
}

func TestSession_CRUD(t *testing.T) {
	ctx := context.Background()
	db := openTestDB(t)

	id := mustIdent(t, "h:/p:a1b2c3d4-5678-4abc-9def-0123456789ab")

	sess, err := db.SessionStart(ctx, id, json.RawMessage(`{"mode":"flow"}`))
	if err != nil {
		t.Fatalf("SessionStart: %v", err)
	}
	if sess.State != "active" {
		t.Errorf("state = %q, want active", sess.State)
	}
	var data map[string]any
	if err := json.Unmarshal(sess.Data, &data); err != nil {
		t.Fatalf("decode data: %v", err)
	}
	if data["mode"] != "flow" {
		t.Errorf("data[mode] = %v, want flow", data["mode"])
	}

	if _, err := db.SessionTouch(ctx, id); err != nil {
		t.Fatalf("SessionTouch: %v", err)
	}
	got, err := db.SessionGet(ctx, id)
	if err != nil {
		t.Fatalf("SessionGet: %v", err)
	}
	if got == nil {
		t.Fatal("SessionGet returned nil")
	}

	stopped, err := db.SessionStop(ctx, id)
	if err != nil {
		t.Fatalf("SessionStop: %v", err)
	}
	if stopped.State != "stopped" {
		t.Errorf("state = %q, want stopped", stopped.State)
	}

	list, err := db.SessionList(ctx, "h", "/p", "", 0)
	if err != nil {
		t.Fatalf("SessionList: %v", err)
	}
	if len(list) != 1 {
		t.Fatalf("len(list) = %d, want 1", len(list))
	}

	projects, err := db.ProjectList(ctx, "")
	if err != nil {
		t.Fatalf("ProjectList: %v", err)
	}
	if len(projects) != 1 || projects[0].Host != "h" || projects[0].ProjectDir != "/p" {
		t.Errorf("ProjectList = %+v", projects)
	}
}

func TestState_PutIncrResetDelete(t *testing.T) {
	ctx := context.Background()
	db := openTestDB(t)
	id := mustIdent(t, "h:/p:a1b2c3d4-5678-4abc-9def-0123456789ab")

	if _, err := db.StatePut(ctx, id, json.RawMessage(`{"phase":"plan"}`), false); err != nil {
		t.Fatalf("StatePut: %v", err)
	}
	if _, err := db.StatePut(ctx, id, json.RawMessage(`{"step":"1"}`), true); err != nil {
		t.Fatalf("StatePut merge: %v", err)
	}

	sess, err := db.SessionGet(ctx, id)
	if err != nil {
		t.Fatalf("SessionGet: %v", err)
	}
	var data map[string]any
	if err := json.Unmarshal(sess.Data, &data); err != nil {
		t.Fatalf("decode data: %v", err)
	}
	if data["phase"] != "plan" || data["step"] != "1" {
		t.Errorf("data = %v, want {phase:plan step:1}", data)
	}

	n, err := db.StateIncr(ctx, id, "tool_calls", 1)
	if err != nil {
		t.Fatalf("StateIncr: %v", err)
	}
	if n != 1 {
		t.Errorf("incr 1 = %d, want 1", n)
	}
	n, err = db.StateIncr(ctx, id, "tool_calls", 2)
	if err != nil {
		t.Fatalf("StateIncr: %v", err)
	}
	if n != 3 {
		t.Errorf("incr 2 = %d, want 3", n)
	}

	n, err = db.StateReset(ctx, id, "tool_calls", 0)
	if err != nil {
		t.Fatalf("StateReset: %v", err)
	}
	if n != 0 {
		t.Errorf("reset = %d, want 0", n)
	}

	status, err := db.StatusGet(ctx, id)
	if err != nil {
		t.Fatalf("StatusGet: %v", err)
	}
	if status.Phase != "plan" {
		t.Errorf("status.Phase = %q, want plan", status.Phase)
	}
	if status.ToolCalls != 0 {
		t.Errorf("status.ToolCalls = %d, want 0", status.ToolCalls)
	}

	ok, err := db.StateDelete(ctx, id)
	if err != nil {
		t.Fatalf("StateDelete: %v", err)
	}
	if !ok {
		t.Error("StateDelete returned false")
	}
	got, _ := db.SessionGet(ctx, id)
	if got != nil {
		t.Error("session still present after delete")
	}
}

func TestStorage_SessionScope(t *testing.T) {
	ctx := context.Background()
	db := openTestDB(t)
	id := mustIdent(t, "h:/p:a1b2c3d4-5678-4abc-9def-0123456789ab")

	if _, err := db.StoragePut(ctx, id, "k1", "v1"); err != nil {
		t.Fatalf("StoragePut: %v", err)
	}
	blob, err := db.StorageGet(ctx, id, "k1")
	if err != nil {
		t.Fatalf("StorageGet: %v", err)
	}
	if blob == nil || blob.Content != "v1" {
		t.Errorf("blob = %+v", blob)
	}
	if _, err := db.StoragePut(ctx, id, "k1", "v2"); err != nil {
		t.Fatalf("StoragePut update: %v", err)
	}
	blob, _ = db.StorageGet(ctx, id, "k1")
	if blob.Content != "v2" {
		t.Errorf("update blob = %+v", blob)
	}

	if _, err := db.StoragePut(ctx, id, "k2", "v2"); err != nil {
		t.Fatalf("StoragePut k2: %v", err)
	}
	items, err := db.StorageList(ctx, id)
	if err != nil {
		t.Fatalf("StorageList: %v", err)
	}
	if len(items) != 2 {
		t.Fatalf("len(items) = %d, want 2", len(items))
	}

	ok, err := db.StorageDelete(ctx, id, "k1")
	if err != nil || !ok {
		t.Fatalf("StorageDelete: ok=%v err=%v", ok, err)
	}
}

func TestStorage_ProjectScope(t *testing.T) {
	ctx := context.Background()
	db := openTestDB(t)
	id := mustIdent(t, "h:/p")

	if _, err := db.StoragePut(ctx, id, "proj-k", "x"); err != nil {
		t.Fatalf("StoragePut: %v", err)
	}
	blob, err := db.StorageGet(ctx, id, "proj-k")
	if err != nil {
		t.Fatalf("StorageGet: %v", err)
	}
	if blob == nil || blob.Content != "x" {
		t.Errorf("blob = %+v", blob)
	}
	items, _ := db.StorageList(ctx, id)
	if len(items) != 1 {
		t.Errorf("len(items) = %d, want 1", len(items))
	}

	// Session-scoped and project-scoped keys are independent.
	sid := mustIdent(t, "h:/p:a1b2c3d4-5678-4abc-9def-0123456789ab")
	if _, err := db.StoragePut(ctx, sid, "proj-k", "session-value"); err != nil {
		t.Fatalf("StoragePut session: %v", err)
	}
	projBlob, _ := db.StorageGet(ctx, id, "proj-k")
	if projBlob.Content != "x" {
		t.Errorf("project blob polluted by session write: %+v", projBlob)
	}
}

func TestLogs_AppendQuery(t *testing.T) {
	ctx := context.Background()
	db := openTestDB(t)
	id := mustIdent(t, "h:/p:a1b2c3d4-5678-4abc-9def-0123456789ab")

	for i := 0; i < 3; i++ {
		if _, err := db.LogAppend(ctx, id, "hook.test", json.RawMessage(`{"i":0}`)); err != nil {
			t.Fatalf("LogAppend: %v", err)
		}
	}
	rows, err := db.LogQuery(ctx, "h", "/p", id.SessionID, 0)
	if err != nil {
		t.Fatalf("LogQuery: %v", err)
	}
	if len(rows) != 3 {
		t.Fatalf("len(rows) = %d, want 3", len(rows))
	}
	for _, r := range rows {
		if r.Event != "hook.test" {
			t.Errorf("event = %q", r.Event)
		}
		var p map[string]any
		if err := json.Unmarshal(r.Payload, &p); err != nil {
			t.Errorf("payload decode: %v", err)
		}
	}

	// Project-level (session_id=NULL) filter.
	projID := mustIdent(t, "h:/p")
	if _, err := db.LogAppend(ctx, projID, "project.event", nil); err != nil {
		t.Fatalf("LogAppend project: %v", err)
	}
	allRows, _ := db.LogQuery(ctx, "h", "/p", "", 0)
	if len(allRows) != 4 {
		t.Errorf("all rows = %d, want 4", len(allRows))
	}
}

func TestSessionStart_JSONRoundTrip(t *testing.T) {
	ctx := context.Background()
	db := openTestDB(t)
	id := mustIdent(t, "h:/p:a1b2c3d4-5678-4abc-9def-0123456789ab")

	payload := json.RawMessage(`{"nested":{"k":"v"},"arr":[1,2,3]}`)
	if _, err := db.SessionStart(ctx, id, payload); err != nil {
		t.Fatalf("SessionStart: %v", err)
	}
	sess, _ := db.SessionGet(ctx, id)
	var got struct {
		Nested map[string]string `json:"nested"`
		Arr    []int             `json:"arr"`
	}
	if err := json.Unmarshal(sess.Data, &got); err != nil {
		t.Fatalf("decode data: %v", err)
	}
	if got.Nested["k"] != "v" {
		t.Errorf("nested.k = %q", got.Nested["k"])
	}
	if len(got.Arr) != 3 {
		t.Errorf("arr len = %d", len(got.Arr))
	}
}
