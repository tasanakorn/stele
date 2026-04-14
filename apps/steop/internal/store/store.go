package store

import (
	"context"
	"database/sql"
	"errors"
	"fmt"
	"net/url"
	"os"

	_ "modernc.org/sqlite"
)

var errSchemaNewer = errors.New("steop: on-disk schema is newer than this binary")

// ErrSchemaNewer is the exported alias so callers outside the package can
// detect downgrade scenarios.
var ErrSchemaNewer = errSchemaNewer

type DB struct {
	sql *sql.DB
}

// Open opens (or creates) a SQLite DB at path with the DSN pragmas documented
// in PRD-020 §4.2 and runs any pending migrations.
func Open(path string) (*DB, error) {
	dsn := buildDSN(path)
	sdb, err := sql.Open("sqlite", dsn)
	if err != nil {
		return nil, fmt.Errorf("sql.Open: %w", err)
	}
	// Serialize writes so BEGIN IMMEDIATE queues cleanly on the RESERVED
	// lock; readers still get the WAL fast path because we re-open read-only
	// conns under a single writer pool. MaxOpenConns=1 matches the per-hook
	// process model where contention is across processes, not goroutines.
	sdb.SetMaxOpenConns(1)
	if err := sdb.Ping(); err != nil {
		sdb.Close()
		return nil, fmt.Errorf("ping: %w", err)
	}
	db := &DB{sql: sdb}
	if err := Migrate(db); err != nil {
		sdb.Close()
		return nil, err
	}
	return db, nil
}

// OpenIfExists opens the DB only if the file already exists on disk. Returns
// (nil, nil) when the file is absent so statusline / read-only hot paths can
// skip the cold-create cost per PRD-020 §4.9.
func OpenIfExists(path string) (*DB, error) {
	if _, err := os.Stat(path); err != nil {
		if os.IsNotExist(err) {
			return nil, nil
		}
		return nil, err
	}
	return Open(path)
}

// Close closes the underlying *sql.DB handle.
func (d *DB) Close() error {
	if d == nil || d.sql == nil {
		return nil
	}
	return d.sql.Close()
}

// SQL exposes the underlying *sql.DB for tests and low-level callers.
func (d *DB) SQL() *sql.DB { return d.sql }

// Tx wraps a checked-out sql.Conn that holds a BEGIN IMMEDIATE transaction.
// Call Commit or Rollback exactly once; both release the underlying conn.
type Tx struct {
	ctx  context.Context
	conn *sql.Conn
	done bool
}

// BeginImmediate starts a BEGIN IMMEDIATE transaction. The returned *Tx
// behaves like sql.Tx for ExecContext / QueryRowContext / QueryContext, but
// it holds the RESERVED lock up front so concurrent hook processes queue on
// the 5-second busy_timeout rather than failing mid-RMW.
func (d *DB) BeginImmediate(ctx context.Context) (*Tx, error) {
	conn, err := d.sql.Conn(ctx)
	if err != nil {
		return nil, err
	}
	if _, err := conn.ExecContext(ctx, "BEGIN IMMEDIATE"); err != nil {
		conn.Close()
		return nil, err
	}
	return &Tx{ctx: ctx, conn: conn}, nil
}

func (t *Tx) Commit() error {
	if t.done {
		return nil
	}
	t.done = true
	_, err := t.conn.ExecContext(t.ctx, "COMMIT")
	t.conn.Close()
	return err
}

func (t *Tx) Rollback() error {
	if t.done {
		return nil
	}
	t.done = true
	_, err := t.conn.ExecContext(t.ctx, "ROLLBACK")
	t.conn.Close()
	return err
}

func (t *Tx) ExecContext(ctx context.Context, query string, args ...any) (sql.Result, error) {
	return t.conn.ExecContext(ctx, query, args...)
}

func (t *Tx) QueryRowContext(ctx context.Context, query string, args ...any) *sql.Row {
	return t.conn.QueryRowContext(ctx, query, args...)
}

func (t *Tx) QueryContext(ctx context.Context, query string, args ...any) (*sql.Rows, error) {
	return t.conn.QueryContext(ctx, query, args...)
}

func buildDSN(path string) string {
	q := url.Values{}
	q.Add("_pragma", "journal_mode(WAL)")
	q.Add("_pragma", "busy_timeout(5000)")
	q.Add("_pragma", "synchronous(NORMAL)")
	q.Add("_pragma", "foreign_keys(ON)")
	q.Add("_pragma", "temp_store(MEMORY)")
	return "file:" + path + "?" + q.Encode()
}
