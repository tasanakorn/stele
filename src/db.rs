use rusqlite::{params, Connection};
use std::sync::Arc;
use tokio::sync::Mutex;

use crate::models::{Memory, MemoryType, ScopeInfo, SearchResult, TagInfo};
use crate::query::SearchParams;

pub type DbPool = Arc<Mutex<Connection>>;

pub fn init_db(path: &str) -> rusqlite::Result<DbPool> {
    let conn = Connection::open(path)?;

    conn.execute_batch("PRAGMA journal_mode=WAL;")?;
    conn.execute_batch("PRAGMA foreign_keys=ON;")?;

    conn.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS memories (
            id          TEXT PRIMARY KEY,
            title       TEXT NOT NULL,
            content     TEXT NOT NULL,
            memory_type TEXT NOT NULL DEFAULT 'knowledge',
            scope       TEXT NOT NULL DEFAULT '',
            author      TEXT,
            created_at  TEXT NOT NULL,
            updated_at  TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS memory_tags (
            memory_id TEXT NOT NULL REFERENCES memories(id) ON DELETE CASCADE,
            tag       TEXT NOT NULL,
            PRIMARY KEY (memory_id, tag)
        );

        CREATE INDEX IF NOT EXISTS idx_memories_scope ON memories(scope);
        CREATE INDEX IF NOT EXISTS idx_memory_tags_tag ON memory_tags(tag);

        CREATE VIRTUAL TABLE IF NOT EXISTS memories_fts USING fts5(
            title,
            content,
            content='memories',
            content_rowid='rowid'
        );

        -- Triggers to keep FTS in sync
        CREATE TRIGGER IF NOT EXISTS memories_ai AFTER INSERT ON memories BEGIN
            INSERT INTO memories_fts(rowid, title, content)
            VALUES (new.rowid, new.title, new.content);
        END;

        CREATE TRIGGER IF NOT EXISTS memories_ad AFTER DELETE ON memories BEGIN
            INSERT INTO memories_fts(memories_fts, rowid, title, content)
            VALUES ('delete', old.rowid, old.title, old.content);
        END;

        CREATE TRIGGER IF NOT EXISTS memories_au AFTER UPDATE ON memories BEGIN
            INSERT INTO memories_fts(memories_fts, rowid, title, content)
            VALUES ('delete', old.rowid, old.title, old.content);
            INSERT INTO memories_fts(rowid, title, content)
            VALUES (new.rowid, new.title, new.content);
        END;
        ",
    )?;

    Ok(Arc::new(Mutex::new(conn)))
}

pub fn insert_memory(conn: &Connection, memory: &Memory) -> rusqlite::Result<()> {
    conn.execute(
        "INSERT INTO memories (id, title, content, memory_type, scope, author, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        params![
            memory.id,
            memory.title,
            memory.content,
            memory.memory_type.as_str(),
            memory.scope,
            memory.author,
            memory.created_at,
            memory.updated_at,
        ],
    )?;

    for tag in &memory.tags {
        conn.execute(
            "INSERT INTO memory_tags (memory_id, tag) VALUES (?1, ?2)",
            params![memory.id, tag],
        )?;
    }

    Ok(())
}

pub fn get_memory(conn: &Connection, id: &str) -> rusqlite::Result<Option<Memory>> {
    let mut stmt = conn.prepare(
        "SELECT id, title, content, memory_type, scope, author, created_at, updated_at
         FROM memories WHERE id = ?1",
    )?;

    let memory = stmt
        .query_row(params![id], |row| {
            Ok(Memory {
                id: row.get(0)?,
                title: row.get(1)?,
                content: row.get(2)?,
                memory_type: MemoryType::from_str(&row.get::<_, String>(3)?),
                scope: row.get(4)?,
                author: row.get(5)?,
                tags: Vec::new(),
                created_at: row.get(6)?,
                updated_at: row.get(7)?,
            })
        })
        .optional()?;

    match memory {
        Some(mut m) => {
            m.tags = get_tags(conn, &m.id)?;
            Ok(Some(m))
        }
        None => Ok(None),
    }
}

fn get_tags(conn: &Connection, memory_id: &str) -> rusqlite::Result<Vec<String>> {
    let mut stmt = conn.prepare("SELECT tag FROM memory_tags WHERE memory_id = ?1 ORDER BY tag")?;
    let tags = stmt
        .query_map(params![memory_id], |row| row.get(0))?
        .collect::<rusqlite::Result<Vec<String>>>()?;
    Ok(tags)
}

pub fn update_memory(
    conn: &Connection,
    id: &str,
    title: Option<&str>,
    content: Option<&str>,
    scope: Option<&str>,
    tags: Option<&[String]>,
    memory_type: Option<&str>,
) -> rusqlite::Result<bool> {
    let exists: bool =
        conn.query_row("SELECT COUNT(*) FROM memories WHERE id = ?1", params![id], |row| {
            Ok(row.get::<_, i64>(0)? > 0)
        })?;

    if !exists {
        return Ok(false);
    }

    let now = chrono::Utc::now().to_rfc3339();

    if let Some(t) = title {
        conn.execute(
            "UPDATE memories SET title = ?1, updated_at = ?2 WHERE id = ?3",
            params![t, now, id],
        )?;
    }
    if let Some(c) = content {
        conn.execute(
            "UPDATE memories SET content = ?1, updated_at = ?2 WHERE id = ?3",
            params![c, now, id],
        )?;
    }
    if let Some(s) = scope {
        conn.execute(
            "UPDATE memories SET scope = ?1, updated_at = ?2 WHERE id = ?3",
            params![s, now, id],
        )?;
    }
    if let Some(mt) = memory_type {
        conn.execute(
            "UPDATE memories SET memory_type = ?1, updated_at = ?2 WHERE id = ?3",
            params![mt, now, id],
        )?;
    }
    if let Some(new_tags) = tags {
        conn.execute("DELETE FROM memory_tags WHERE memory_id = ?1", params![id])?;
        for tag in new_tags {
            conn.execute(
                "INSERT INTO memory_tags (memory_id, tag) VALUES (?1, ?2)",
                params![id, tag],
            )?;
        }
        conn.execute(
            "UPDATE memories SET updated_at = ?1 WHERE id = ?2",
            params![now, id],
        )?;
    }

    Ok(true)
}

pub fn delete_memory(conn: &Connection, id: &str) -> rusqlite::Result<bool> {
    let rows = conn.execute("DELETE FROM memories WHERE id = ?1", params![id])?;
    Ok(rows > 0)
}

pub fn search_memories(conn: &Connection, params: &SearchParams) -> rusqlite::Result<Vec<SearchResult>> {
    let limit = params.limit.unwrap_or(20).min(100);

    if params.query.is_some() {
        search_with_fts(conn, params, limit)
    } else {
        search_without_fts(conn, params, limit)
    }
}

fn search_with_fts(
    conn: &Connection,
    params: &SearchParams,
    limit: usize,
) -> rusqlite::Result<Vec<SearchResult>> {
    let query = params.query.as_deref().unwrap_or("");

    let mut sql = String::from(
        "SELECT m.id, m.title, m.content, m.memory_type, m.scope, m.author,
                m.created_at, m.updated_at, fts.rank
         FROM memories_fts fts
         JOIN memories m ON m.rowid = fts.rowid
         WHERE memories_fts MATCH ?1",
    );
    let mut sql_params: Vec<Box<dyn rusqlite::types::ToSql>> = vec![Box::new(query.to_string())];
    let mut param_idx = 2;

    append_scope_filter(&mut sql, &mut sql_params, &mut param_idx, params.scope.as_deref());
    append_tag_filter(&mut sql, &mut sql_params, &mut param_idx, &params.tags, params.match_all_tags);

    sql.push_str(&format!(" ORDER BY fts.rank LIMIT ?{param_idx}"));
    sql_params.push(Box::new(limit as i64));

    let mut stmt = conn.prepare(&sql)?;
    let param_refs: Vec<&dyn rusqlite::types::ToSql> = sql_params.iter().map(|p| p.as_ref()).collect();

    let rows = stmt.query_map(param_refs.as_slice(), |row| {
        Ok(SearchResult {
            id: row.get(0)?,
            title: row.get(1)?,
            content: row.get(2)?,
            memory_type: MemoryType::from_str(&row.get::<_, String>(3)?),
            scope: row.get(4)?,
            author: row.get(5)?,
            tags: Vec::new(),
            created_at: row.get(6)?,
            updated_at: row.get(7)?,
            rank: row.get(8)?,
        })
    })?;

    let mut results = Vec::new();
    for row in rows {
        let mut r = row?;
        r.tags = get_tags(conn, &r.id)?;
        results.push(r);
    }
    Ok(results)
}

fn search_without_fts(
    conn: &Connection,
    params: &SearchParams,
    limit: usize,
) -> rusqlite::Result<Vec<SearchResult>> {
    let mut sql = String::from(
        "SELECT m.id, m.title, m.content, m.memory_type, m.scope, m.author, m.created_at, m.updated_at
         FROM memories m WHERE 1=1",
    );
    let mut sql_params: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();
    let mut param_idx = 1;

    append_scope_filter(&mut sql, &mut sql_params, &mut param_idx, params.scope.as_deref());
    append_tag_filter(&mut sql, &mut sql_params, &mut param_idx, &params.tags, params.match_all_tags);

    sql.push_str(&format!(" ORDER BY updated_at DESC LIMIT ?{param_idx}"));
    sql_params.push(Box::new(limit as i64));

    let mut stmt = conn.prepare(&sql)?;
    let param_refs: Vec<&dyn rusqlite::types::ToSql> = sql_params.iter().map(|p| p.as_ref()).collect();

    let rows = stmt.query_map(param_refs.as_slice(), |row| {
        Ok(SearchResult {
            id: row.get(0)?,
            title: row.get(1)?,
            content: row.get(2)?,
            memory_type: MemoryType::from_str(&row.get::<_, String>(3)?),
            scope: row.get(4)?,
            author: row.get(5)?,
            tags: Vec::new(),
            created_at: row.get(6)?,
            updated_at: row.get(7)?,
            rank: None,
        })
    })?;

    let mut results = Vec::new();
    for row in rows {
        let mut r = row?;
        r.tags = get_tags(conn, &r.id)?;
        results.push(r);
    }
    Ok(results)
}

fn append_scope_filter(
    sql: &mut String,
    sql_params: &mut Vec<Box<dyn rusqlite::types::ToSql>>,
    param_idx: &mut usize,
    scope: Option<&str>,
) {
    if let Some(s) = scope {
        sql.push_str(&format!(
            " AND (m.scope = ?{idx} OR m.scope LIKE ?{idx2})",
            idx = *param_idx,
            idx2 = *param_idx + 1,
        ));
        sql_params.push(Box::new(s.to_string()));
        sql_params.push(Box::new(format!("{s}/%")));
        *param_idx += 2;
    }
}

fn append_tag_filter(
    sql: &mut String,
    sql_params: &mut Vec<Box<dyn rusqlite::types::ToSql>>,
    param_idx: &mut usize,
    tags: &Option<Vec<String>>,
    match_all: bool,
) {
    if let Some(tags) = tags {
        if tags.is_empty() {
            return;
        }

        // Determine the id column reference based on context
        // In FTS queries we use m.id, in non-FTS we use id directly
        let id_col = "m.id";

        if match_all {
            // Intersection: memory must have ALL specified tags
            let placeholders: Vec<String> = tags
                .iter()
                .enumerate()
                .map(|(i, _)| format!("?{}", *param_idx + i))
                .collect();
            sql.push_str(&format!(
                " AND (SELECT COUNT(DISTINCT tag) FROM memory_tags WHERE memory_id = {id_col} AND tag IN ({})) = {}",
                placeholders.join(", "),
                tags.len()
            ));
            for tag in tags {
                sql_params.push(Box::new(tag.clone()));
            }
            *param_idx += tags.len();
        } else {
            // Union: memory must have ANY of the specified tags
            let placeholders: Vec<String> = tags
                .iter()
                .enumerate()
                .map(|(i, _)| format!("?{}", *param_idx + i))
                .collect();
            sql.push_str(&format!(
                " AND {id_col} IN (SELECT memory_id FROM memory_tags WHERE tag IN ({}))",
                placeholders.join(", ")
            ));
            for tag in tags {
                sql_params.push(Box::new(tag.clone()));
            }
            *param_idx += tags.len();
        }
    }
}

pub fn list_scopes(conn: &Connection, prefix: Option<&str>) -> rusqlite::Result<Vec<ScopeInfo>> {
    let (sql, sql_params): (String, Vec<Box<dyn rusqlite::types::ToSql>>) = match prefix {
        Some(p) => (
            "SELECT scope, COUNT(*) as count FROM memories WHERE scope = ?1 OR scope LIKE ?2 GROUP BY scope ORDER BY scope"
                .to_string(),
            vec![Box::new(p.to_string()), Box::new(format!("{p}/%"))],
        ),
        None => (
            "SELECT scope, COUNT(*) as count FROM memories GROUP BY scope ORDER BY scope".to_string(),
            vec![],
        ),
    };

    let mut stmt = conn.prepare(&sql)?;
    let param_refs: Vec<&dyn rusqlite::types::ToSql> = sql_params.iter().map(|p| p.as_ref()).collect();

    let rows = stmt.query_map(param_refs.as_slice(), |row| {
        Ok(ScopeInfo {
            scope: row.get(0)?,
            count: row.get(1)?,
        })
    })?;

    rows.collect()
}

pub fn list_tags(conn: &Connection, scope: Option<&str>) -> rusqlite::Result<Vec<TagInfo>> {
    let (sql, sql_params): (String, Vec<Box<dyn rusqlite::types::ToSql>>) = match scope {
        Some(s) => (
            "SELECT mt.tag, COUNT(*) as count
             FROM memory_tags mt
             JOIN memories m ON m.id = mt.memory_id
             WHERE m.scope = ?1 OR m.scope LIKE ?2
             GROUP BY mt.tag ORDER BY count DESC, mt.tag"
                .to_string(),
            vec![Box::new(s.to_string()), Box::new(format!("{s}/%"))],
        ),
        None => (
            "SELECT tag, COUNT(*) as count FROM memory_tags GROUP BY tag ORDER BY count DESC, tag"
                .to_string(),
            vec![],
        ),
    };

    let mut stmt = conn.prepare(&sql)?;
    let param_refs: Vec<&dyn rusqlite::types::ToSql> = sql_params.iter().map(|p| p.as_ref()).collect();

    let rows = stmt.query_map(param_refs.as_slice(), |row| {
        Ok(TagInfo {
            tag: row.get(0)?,
            count: row.get(1)?,
        })
    })?;

    rows.collect()
}

trait OptionalRow {
    fn optional(self) -> rusqlite::Result<Option<Memory>>;
}

impl OptionalRow for rusqlite::Result<Memory> {
    fn optional(self) -> rusqlite::Result<Option<Memory>> {
        match self {
            Ok(m) => Ok(Some(m)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e),
        }
    }
}
