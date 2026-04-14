package store

import (
	"context"
	"database/sql"
	"time"
)

// Blob is a stored key/value entry.
type Blob struct {
	Host       string
	ProjectDir string
	SessionID  string // "" for project-scoped entries
	Key        string
	Content    string
	CreatedAt  int64
	UpdatedAt  int64
}

// BlobListItem is a lightweight row for list operations.
type BlobListItem struct {
	Key       string
	UpdatedAt int64
	Size      int64
}

// BlobMeta is returned by StoragePut.
type BlobMeta struct {
	Key       string
	UpdatedAt int64
}

// StoragePut upserts a blob. 2-segment id → project table, 3-segment →
// session table.
func (d *DB) StoragePut(ctx context.Context, id Identity, key, content string) (*BlobMeta, error) {
	now := time.Now().Unix()
	if id.IsProject() {
		if _, err := d.sql.ExecContext(ctx,
			`INSERT INTO steop_storage_project
			 (host, project_dir, key, content, created_at, updated_at)
			 VALUES (?, ?, ?, ?, ?, ?)
			 ON CONFLICT(host, project_dir, key) DO UPDATE SET
			   content=excluded.content, updated_at=excluded.updated_at`,
			id.Host, id.ProjectDir, key, content, now, now,
		); err != nil {
			return nil, err
		}
	} else {
		if _, err := d.sql.ExecContext(ctx,
			`INSERT INTO steop_storage_session
			 (host, project_dir, session_id, key, content, created_at, updated_at)
			 VALUES (?, ?, ?, ?, ?, ?, ?)
			 ON CONFLICT(host, project_dir, session_id, key) DO UPDATE SET
			   content=excluded.content, updated_at=excluded.updated_at`,
			id.Host, id.ProjectDir, id.SessionID, key, content, now, now,
		); err != nil {
			return nil, err
		}
	}
	return &BlobMeta{Key: key, UpdatedAt: now}, nil
}

// StorageGet returns the blob or (nil, nil) if not found.
func (d *DB) StorageGet(ctx context.Context, id Identity, key string) (*Blob, error) {
	b := &Blob{Host: id.Host, ProjectDir: id.ProjectDir, SessionID: id.SessionID, Key: key}
	var err error
	if id.IsProject() {
		err = d.sql.QueryRowContext(ctx,
			`SELECT content, created_at, updated_at FROM steop_storage_project
			 WHERE host=? AND project_dir=? AND key=?`,
			id.Host, id.ProjectDir, key,
		).Scan(&b.Content, &b.CreatedAt, &b.UpdatedAt)
	} else {
		err = d.sql.QueryRowContext(ctx,
			`SELECT content, created_at, updated_at FROM steop_storage_session
			 WHERE host=? AND project_dir=? AND session_id=? AND key=?`,
			id.Host, id.ProjectDir, id.SessionID, key,
		).Scan(&b.Content, &b.CreatedAt, &b.UpdatedAt)
	}
	if err == sql.ErrNoRows {
		return nil, nil
	}
	if err != nil {
		return nil, err
	}
	return b, nil
}

// StorageDelete removes a blob. Returns true if a row was deleted.
func (d *DB) StorageDelete(ctx context.Context, id Identity, key string) (bool, error) {
	var res sql.Result
	var err error
	if id.IsProject() {
		res, err = d.sql.ExecContext(ctx,
			`DELETE FROM steop_storage_project WHERE host=? AND project_dir=? AND key=?`,
			id.Host, id.ProjectDir, key)
	} else {
		res, err = d.sql.ExecContext(ctx,
			`DELETE FROM steop_storage_session
			 WHERE host=? AND project_dir=? AND session_id=? AND key=?`,
			id.Host, id.ProjectDir, id.SessionID, key)
	}
	if err != nil {
		return false, err
	}
	n, _ := res.RowsAffected()
	return n > 0, nil
}

// StorageList lists keys in the given scope, ordered by key.
func (d *DB) StorageList(ctx context.Context, id Identity) ([]BlobListItem, error) {
	var rows *sql.Rows
	var err error
	if id.IsProject() {
		rows, err = d.sql.QueryContext(ctx,
			`SELECT key, updated_at, LENGTH(content) FROM steop_storage_project
			 WHERE host=? AND project_dir=? ORDER BY key`,
			id.Host, id.ProjectDir)
	} else {
		rows, err = d.sql.QueryContext(ctx,
			`SELECT key, updated_at, LENGTH(content) FROM steop_storage_session
			 WHERE host=? AND project_dir=? AND session_id=? ORDER BY key`,
			id.Host, id.ProjectDir, id.SessionID)
	}
	if err != nil {
		return nil, err
	}
	defer rows.Close()
	var out []BlobListItem
	for rows.Next() {
		var it BlobListItem
		if err := rows.Scan(&it.Key, &it.UpdatedAt, &it.Size); err != nil {
			return nil, err
		}
		out = append(out, it)
	}
	return out, rows.Err()
}
