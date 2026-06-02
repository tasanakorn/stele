use rusqlite::{params, Connection};
use std::sync::Arc;
use tokio::sync::Mutex;

use stele_common::models::{
    Entity, EntitySearchResult, Graph, Memory, MemoryType, Observation, RecentMemorySummary,
    Relation, ScopeInfo, SearchResult, Stats, TagInfo,
};
use stele_common::query::SearchParams;

pub type DbPool = Arc<Mutex<Connection>>;

pub fn init_db(path: &str) -> rusqlite::Result<DbPool> {
    let conn = Connection::open(path)?;

    conn.execute_batch("PRAGMA journal_mode=WAL;")?;
    conn.execute_batch("PRAGMA foreign_keys=ON;")?;

    // Drop any v0.5 steop_* tables before creating the new v0.6 schema so
    // old steop_logs rows with the old column layout get wiped cleanly.
    ensure_steop_schema(&conn)?;

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

        -- Knowledge Graph tables
        CREATE TABLE IF NOT EXISTS entities (
            id          TEXT PRIMARY KEY,
            name        TEXT NOT NULL,
            entity_type TEXT NOT NULL,
            scope       TEXT NOT NULL DEFAULT '',
            created_at  TEXT NOT NULL,
            updated_at  TEXT NOT NULL,
            UNIQUE(name, scope)
        );
        CREATE INDEX IF NOT EXISTS idx_entities_scope ON entities(scope);
        CREATE INDEX IF NOT EXISTS idx_entities_name ON entities(name);

        CREATE TABLE IF NOT EXISTS observations (
            id         TEXT PRIMARY KEY,
            entity_id  TEXT NOT NULL REFERENCES entities(id) ON DELETE CASCADE,
            content    TEXT NOT NULL,
            created_at TEXT NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_observations_entity ON observations(entity_id);

        CREATE TABLE IF NOT EXISTS relations (
            id            TEXT PRIMARY KEY,
            from_entity   TEXT NOT NULL REFERENCES entities(id) ON DELETE CASCADE,
            to_entity     TEXT NOT NULL REFERENCES entities(id) ON DELETE CASCADE,
            relation_type TEXT NOT NULL,
            scope         TEXT NOT NULL DEFAULT '',
            created_at    TEXT NOT NULL,
            UNIQUE(from_entity, to_entity, relation_type)
        );
        CREATE INDEX IF NOT EXISTS idx_relations_from ON relations(from_entity);
        CREATE INDEX IF NOT EXISTS idx_relations_to ON relations(to_entity);

        -- FTS for entity names/types
        CREATE VIRTUAL TABLE IF NOT EXISTS entities_fts USING fts5(
            name, entity_type, content='entities', content_rowid='rowid'
        );
        CREATE TRIGGER IF NOT EXISTS entities_ai AFTER INSERT ON entities BEGIN
            INSERT INTO entities_fts(rowid, name, entity_type)
            VALUES (new.rowid, new.name, new.entity_type);
        END;
        CREATE TRIGGER IF NOT EXISTS entities_ad AFTER DELETE ON entities BEGIN
            INSERT INTO entities_fts(entities_fts, rowid, name, entity_type)
            VALUES ('delete', old.rowid, old.name, old.entity_type);
        END;
        CREATE TRIGGER IF NOT EXISTS entities_au AFTER UPDATE ON entities BEGIN
            INSERT INTO entities_fts(entities_fts, rowid, name, entity_type)
            VALUES ('delete', old.rowid, old.name, old.entity_type);
            INSERT INTO entities_fts(rowid, name, entity_type)
            VALUES (new.rowid, new.name, new.entity_type);
        END;

        -- FTS for observation content
        CREATE VIRTUAL TABLE IF NOT EXISTS observations_fts USING fts5(
            content, content='observations', content_rowid='rowid'
        );
        CREATE TRIGGER IF NOT EXISTS observations_ai AFTER INSERT ON observations BEGIN
            INSERT INTO observations_fts(rowid, content)
            VALUES (new.rowid, new.content);
        END;
        CREATE TRIGGER IF NOT EXISTS observations_ad AFTER DELETE ON observations BEGIN
            INSERT INTO observations_fts(observations_fts, rowid, content)
            VALUES ('delete', old.rowid, old.content);
        END;
        CREATE TRIGGER IF NOT EXISTS observations_au AFTER UPDATE ON observations BEGIN
            INSERT INTO observations_fts(observations_fts, rowid, content)
            VALUES ('delete', old.rowid, old.content);
            INSERT INTO observations_fts(rowid, content)
            VALUES (new.rowid, new.content);
        END;

        CREATE TABLE IF NOT EXISTS mailbox_inbox (
            message_id   INTEGER PRIMARY KEY AUTOINCREMENT,
            mail_uid     TEXT    NOT NULL UNIQUE,
            to_project   TEXT    NOT NULL,
            attention    TEXT,
            from_addr    TEXT    NOT NULL,
            subject      TEXT    NOT NULL DEFAULT '',
            message_type TEXT    NOT NULL DEFAULT 'NOTE',
            meta         TEXT    NOT NULL DEFAULT '{}',
            payload      TEXT    NOT NULL DEFAULT '{}',
            created_at   TEXT    NOT NULL,
            status       TEXT    NOT NULL DEFAULT 'NEW'
        );
        CREATE UNIQUE INDEX IF NOT EXISTS idx_mailbox_inbox_uid
            ON mailbox_inbox(mail_uid);
        CREATE INDEX IF NOT EXISTS idx_mailbox_inbox_to
            ON mailbox_inbox(to_project, status, created_at);

        CREATE TABLE IF NOT EXISTS mailbox_outbox (
            mail_uid          TEXT    PRIMARY KEY,
            to_host           TEXT    NOT NULL,
            to_project        TEXT    NOT NULL,
            attention         TEXT,
            from_addr         TEXT    NOT NULL,
            subject           TEXT    NOT NULL DEFAULT '',
            message_type      TEXT    NOT NULL DEFAULT 'NOTE',
            meta              TEXT    NOT NULL DEFAULT '{}',
            payload           TEXT    NOT NULL DEFAULT '{}',
            status            TEXT    NOT NULL DEFAULT 'QUEUED',
            attempts          INTEGER NOT NULL DEFAULT 0,
            next_attempt_at   TEXT    NOT NULL,
            last_error        TEXT,
            remote_message_id INTEGER,
            created_at        TEXT    NOT NULL,
            delivered_at      TEXT
        );
        CREATE INDEX IF NOT EXISTS idx_mailbox_outbox_due
            ON mailbox_outbox(status, next_attempt_at);

        CREATE TABLE IF NOT EXISTS mailbox_agent_alias (
            alias        TEXT    NOT NULL,
            project_dir  TEXT    NOT NULL,
            last_seen    TEXT    NOT NULL,
            PRIMARY KEY (project_dir, alias)
        );
        ",
    )?;

    Ok(Arc::new(Mutex::new(conn)))
}

fn ensure_steop_schema(conn: &Connection) -> rusqlite::Result<()> {
    // v0.8.0: drop legacy and pre-v0.8 mailbox tables so the schema-init
    // CREATE TABLE IF NOT EXISTS block below recreates steop_mailbox with the
    // v2 shape. Mailbox data is intentionally not preserved (see docs/prd/prd-001-mailbox-v2.md §9).
    // v0.16.0: drop session/storage/log tables migrated to steop local SQLite.
    conn.execute_batch(
        "DROP TABLE IF EXISTS steop_storage;
         DROP TABLE IF EXISTS steop_state;
         DROP TABLE IF EXISTS steop_counters;
         DROP TABLE IF EXISTS steop_inbox;
         DROP TABLE IF EXISTS steop_logs;
         DROP TABLE IF EXISTS steop_mailbox;
         DROP TABLE IF EXISTS steop_sessions;
         DROP TABLE IF EXISTS steop_storage_session;
         DROP TABLE IF EXISTS steop_storage_project;",
    )?;
    Ok(())
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
    let exists: bool = conn.query_row(
        "SELECT COUNT(*) FROM memories WHERE id = ?1",
        params![id],
        |row| Ok(row.get::<_, i64>(0)? > 0),
    )?;

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

pub fn search_memories(
    conn: &Connection,
    params: &SearchParams,
) -> rusqlite::Result<Vec<SearchResult>> {
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

    append_scope_filter(
        &mut sql,
        &mut sql_params,
        &mut param_idx,
        params.scope.as_deref(),
    );
    append_tag_filter(
        &mut sql,
        &mut sql_params,
        &mut param_idx,
        &params.tags,
        params.match_all_tags,
    );

    sql.push_str(&format!(" ORDER BY fts.rank LIMIT ?{param_idx}"));
    sql_params.push(Box::new(limit as i64));

    let mut stmt = conn.prepare(&sql)?;
    let param_refs: Vec<&dyn rusqlite::types::ToSql> =
        sql_params.iter().map(|p| p.as_ref()).collect();

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

    append_scope_filter(
        &mut sql,
        &mut sql_params,
        &mut param_idx,
        params.scope.as_deref(),
    );
    append_tag_filter(
        &mut sql,
        &mut sql_params,
        &mut param_idx,
        &params.tags,
        params.match_all_tags,
    );

    sql.push_str(&format!(" ORDER BY updated_at DESC LIMIT ?{param_idx}"));
    sql_params.push(Box::new(limit as i64));

    let mut stmt = conn.prepare(&sql)?;
    let param_refs: Vec<&dyn rusqlite::types::ToSql> =
        sql_params.iter().map(|p| p.as_ref()).collect();

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

fn escape_like(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('%', "\\%")
        .replace('_', "\\_")
}

fn append_scope_filter(
    sql: &mut String,
    sql_params: &mut Vec<Box<dyn rusqlite::types::ToSql>>,
    param_idx: &mut usize,
    scopes: Option<&[String]>,
) {
    if let Some(scopes) = scopes {
        if scopes.is_empty() {
            return;
        }
        let mut clauses = Vec::with_capacity(scopes.len());
        for s in scopes {
            clauses.push(format!(
                "(m.scope = ?{idx} OR m.scope LIKE ?{idx2} ESCAPE '\\')",
                idx = *param_idx,
                idx2 = *param_idx + 1,
            ));
            sql_params.push(Box::new(s.to_string()));
            sql_params.push(Box::new(format!("{}/%", escape_like(s))));
            *param_idx += 2;
        }
        sql.push_str(&format!(" AND ({})", clauses.join(" OR ")));
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
            "SELECT scope, COUNT(*) as count FROM memories WHERE scope = ?1 OR scope LIKE ?2 ESCAPE '\\' GROUP BY scope ORDER BY scope"
                .to_string(),
            vec![Box::new(p.to_string()), Box::new(format!("{}/%", escape_like(p)))],
        ),
        None => (
            "SELECT scope, COUNT(*) as count FROM memories GROUP BY scope ORDER BY scope".to_string(),
            vec![],
        ),
    };

    let mut stmt = conn.prepare(&sql)?;
    let param_refs: Vec<&dyn rusqlite::types::ToSql> =
        sql_params.iter().map(|p| p.as_ref()).collect();

    let rows = stmt.query_map(param_refs.as_slice(), |row| {
        Ok(ScopeInfo {
            scope: row.get(0)?,
            count: row.get(1)?,
        })
    })?;

    rows.collect()
}

pub fn list_tags(conn: &Connection, scopes: Option<&[String]>) -> rusqlite::Result<Vec<TagInfo>> {
    let mut sql = String::new();
    let mut sql_params: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();

    match scopes {
        Some(scopes) if !scopes.is_empty() => {
            sql.push_str(
                "SELECT mt.tag, COUNT(*) as count
                 FROM memory_tags mt
                 JOIN memories m ON m.id = mt.memory_id
                 WHERE ",
            );
            let mut clauses = Vec::with_capacity(scopes.len());
            let mut idx = 1;
            for s in scopes {
                clauses.push(format!(
                    "(m.scope = ?{idx} OR m.scope LIKE ?{idx2} ESCAPE '\\')",
                    idx = idx,
                    idx2 = idx + 1,
                ));
                sql_params.push(Box::new(s.to_string()));
                sql_params.push(Box::new(format!("{}/%", escape_like(s))));
                idx += 2;
            }
            sql.push_str(&format!("({})", clauses.join(" OR ")));
            sql.push_str(" GROUP BY mt.tag ORDER BY count DESC, mt.tag");
        }
        _ => {
            sql.push_str(
                "SELECT tag, COUNT(*) as count FROM memory_tags GROUP BY tag ORDER BY count DESC, tag",
            );
        }
    };

    let mut stmt = conn.prepare(&sql)?;
    let param_refs: Vec<&dyn rusqlite::types::ToSql> =
        sql_params.iter().map(|p| p.as_ref()).collect();

    let rows = stmt.query_map(param_refs.as_slice(), |row| {
        Ok(TagInfo {
            tag: row.get(0)?,
            count: row.get(1)?,
        })
    })?;

    rows.collect()
}

pub fn get_stats(conn: &Connection) -> rusqlite::Result<Stats> {
    let total_memories: i64 =
        conn.query_row("SELECT COUNT(*) FROM memories", [], |row| row.get(0))?;
    let total_scopes: i64 =
        conn.query_row("SELECT COUNT(DISTINCT scope) FROM memories", [], |row| {
            row.get(0)
        })?;
    let total_tags: i64 =
        conn.query_row("SELECT COUNT(DISTINCT tag) FROM memory_tags", [], |row| {
            row.get(0)
        })?;

    let mut stmt = conn.prepare(
        "SELECT id, title, scope, updated_at FROM memories ORDER BY updated_at DESC LIMIT 5",
    )?;
    let recent = stmt
        .query_map([], |row| {
            Ok(RecentMemorySummary {
                id: row.get(0)?,
                title: row.get(1)?,
                scope: row.get(2)?,
                updated_at: row.get(3)?,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;

    Ok(Stats {
        total_memories,
        total_scopes,
        total_tags,
        recent_memories: recent,
    })
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

// ── Knowledge Graph functions ──

pub fn resolve_entity_id(
    conn: &Connection,
    name: &str,
    scope: &str,
) -> rusqlite::Result<Option<String>> {
    let mut stmt = conn.prepare("SELECT id FROM entities WHERE name = ?1 AND scope = ?2")?;
    match stmt.query_row(params![name, scope], |row| row.get::<_, String>(0)) {
        Ok(id) => Ok(Some(id)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(e),
    }
}

pub fn insert_entity(
    conn: &Connection,
    name: &str,
    entity_type: &str,
    scope: &str,
    observations: &[String],
) -> rusqlite::Result<(String, bool)> {
    let now = chrono::Utc::now().to_rfc3339();

    // Check if entity already exists
    if let Some(existing_id) = resolve_entity_id(conn, name, scope)? {
        // Add any new observations to existing entity
        if !observations.is_empty() {
            insert_observations_for_entity(conn, &existing_id, observations)?;
        }
        return Ok((existing_id, false));
    }

    let id = ulid::Ulid::new().to_string();
    conn.execute(
        "INSERT INTO entities (id, name, entity_type, scope, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![id, name, entity_type, scope, now, now],
    )?;

    if !observations.is_empty() {
        insert_observations_for_entity(conn, &id, observations)?;
    }

    Ok((id, true))
}

fn insert_observations_for_entity(
    conn: &Connection,
    entity_id: &str,
    observations: &[String],
) -> rusqlite::Result<()> {
    let now = chrono::Utc::now().to_rfc3339();
    for obs in observations {
        let obs_id = ulid::Ulid::new().to_string();
        conn.execute(
            "INSERT INTO observations (id, entity_id, content, created_at) VALUES (?1, ?2, ?3, ?4)",
            params![obs_id, entity_id, obs, now],
        )?;
    }
    conn.execute(
        "UPDATE entities SET updated_at = ?1 WHERE id = ?2",
        params![now, entity_id],
    )?;
    Ok(())
}

pub fn get_entity_by_name(
    conn: &Connection,
    name: &str,
    scope: &str,
) -> rusqlite::Result<Option<Entity>> {
    let mut stmt = conn.prepare(
        "SELECT id, name, entity_type, scope, created_at, updated_at FROM entities WHERE name = ?1 AND scope = ?2",
    )?;

    let entity = match stmt.query_row(params![name, scope], |row| {
        Ok(Entity {
            id: row.get(0)?,
            name: row.get(1)?,
            entity_type: row.get(2)?,
            scope: row.get(3)?,
            observations: Vec::new(),
            created_at: row.get(4)?,
            updated_at: row.get(5)?,
        })
    }) {
        Ok(e) => e,
        Err(rusqlite::Error::QueryReturnedNoRows) => return Ok(None),
        Err(e) => return Err(e),
    };

    let observations = get_observations(conn, &entity.id)?;
    Ok(Some(Entity {
        observations,
        ..entity
    }))
}

fn get_entity_by_id(conn: &Connection, id: &str) -> rusqlite::Result<Option<Entity>> {
    let mut stmt = conn.prepare(
        "SELECT id, name, entity_type, scope, created_at, updated_at FROM entities WHERE id = ?1",
    )?;

    let entity = match stmt.query_row(params![id], |row| {
        Ok(Entity {
            id: row.get(0)?,
            name: row.get(1)?,
            entity_type: row.get(2)?,
            scope: row.get(3)?,
            observations: Vec::new(),
            created_at: row.get(4)?,
            updated_at: row.get(5)?,
        })
    }) {
        Ok(e) => e,
        Err(rusqlite::Error::QueryReturnedNoRows) => return Ok(None),
        Err(e) => return Err(e),
    };

    let observations = get_observations(conn, &entity.id)?;
    Ok(Some(Entity {
        observations,
        ..entity
    }))
}

fn get_observations(conn: &Connection, entity_id: &str) -> rusqlite::Result<Vec<Observation>> {
    let mut stmt = conn.prepare(
        "SELECT id, content, created_at FROM observations WHERE entity_id = ?1 ORDER BY created_at",
    )?;
    let rows = stmt.query_map(params![entity_id], |row| {
        Ok(Observation {
            id: row.get(0)?,
            content: row.get(1)?,
            created_at: row.get(2)?,
        })
    })?;
    rows.collect()
}

pub fn insert_observations(
    conn: &Connection,
    entity_name: &str,
    scope: &str,
    observations: &[String],
) -> rusqlite::Result<Option<Entity>> {
    let entity_id = match resolve_entity_id(conn, entity_name, scope)? {
        Some(id) => id,
        None => return Ok(None),
    };

    insert_observations_for_entity(conn, &entity_id, observations)?;
    get_entity_by_id(conn, &entity_id)
}

pub fn delete_observations(
    conn: &Connection,
    entity_name: &str,
    scope: &str,
    observations: &[String],
) -> rusqlite::Result<Option<Entity>> {
    let entity_id = match resolve_entity_id(conn, entity_name, scope)? {
        Some(id) => id,
        None => return Ok(None),
    };

    for obs in observations {
        conn.execute(
            "DELETE FROM observations WHERE entity_id = ?1 AND content = ?2",
            params![entity_id, obs],
        )?;
    }

    let now = chrono::Utc::now().to_rfc3339();
    conn.execute(
        "UPDATE entities SET updated_at = ?1 WHERE id = ?2",
        params![now, entity_id],
    )?;

    get_entity_by_id(conn, &entity_id)
}

pub fn delete_entity(conn: &Connection, name: &str, scope: &str) -> rusqlite::Result<bool> {
    let rows = conn.execute(
        "DELETE FROM entities WHERE name = ?1 AND scope = ?2",
        params![name, scope],
    )?;
    Ok(rows > 0)
}

pub fn insert_relation(
    conn: &Connection,
    from_name: &str,
    to_name: &str,
    relation_type: &str,
    scope: &str,
) -> rusqlite::Result<Option<(String, bool)>> {
    let from_id = match resolve_entity_id(conn, from_name, scope)? {
        Some(id) => id,
        None => return Ok(None),
    };
    let to_id = match resolve_entity_id(conn, to_name, scope)? {
        Some(id) => id,
        None => return Ok(None),
    };

    // Check if relation already exists
    let existing: bool = conn.query_row(
        "SELECT COUNT(*) FROM relations WHERE from_entity = ?1 AND to_entity = ?2 AND relation_type = ?3",
        params![from_id, to_id, relation_type],
        |row| Ok(row.get::<_, i64>(0)? > 0),
    )?;

    if existing {
        let id: String = conn.query_row(
            "SELECT id FROM relations WHERE from_entity = ?1 AND to_entity = ?2 AND relation_type = ?3",
            params![from_id, to_id, relation_type],
            |row| row.get(0),
        )?;
        return Ok(Some((id, false)));
    }

    let now = chrono::Utc::now().to_rfc3339();
    let id = ulid::Ulid::new().to_string();
    conn.execute(
        "INSERT INTO relations (id, from_entity, to_entity, relation_type, scope, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![id, from_id, to_id, relation_type, scope, now],
    )?;

    Ok(Some((id, true)))
}

pub fn delete_relation(
    conn: &Connection,
    from_name: &str,
    to_name: &str,
    relation_type: &str,
    scope: &str,
) -> rusqlite::Result<bool> {
    let from_id = match resolve_entity_id(conn, from_name, scope)? {
        Some(id) => id,
        None => return Ok(false),
    };
    let to_id = match resolve_entity_id(conn, to_name, scope)? {
        Some(id) => id,
        None => return Ok(false),
    };

    let rows = conn.execute(
        "DELETE FROM relations WHERE from_entity = ?1 AND to_entity = ?2 AND relation_type = ?3",
        params![from_id, to_id, relation_type],
    )?;
    Ok(rows > 0)
}

pub fn read_graph(conn: &Connection, scopes: &[String]) -> rusqlite::Result<Graph> {
    if scopes.is_empty() {
        return Ok(Graph {
            entities: Vec::new(),
            relations: Vec::new(),
        });
    }

    // Build scope filter for entities
    let mut entity_sql = String::from(
        "SELECT id, name, entity_type, scope, created_at, updated_at FROM entities WHERE ",
    );
    let mut sql_params: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();
    let mut idx = 1;
    let mut clauses = Vec::with_capacity(scopes.len());
    for s in scopes {
        clauses.push(format!(
            "(scope = ?{idx} OR scope LIKE ?{idx2} ESCAPE '\\')",
            idx = idx,
            idx2 = idx + 1,
        ));
        sql_params.push(Box::new(s.to_string()));
        sql_params.push(Box::new(format!("{}/%", escape_like(s))));
        idx += 2;
    }
    entity_sql.push_str(&format!("({}) ORDER BY name", clauses.join(" OR ")));

    let mut entity_stmt = conn.prepare(&entity_sql)?;
    let param_refs: Vec<&dyn rusqlite::types::ToSql> =
        sql_params.iter().map(|p| p.as_ref()).collect();
    let entities_raw: Vec<Entity> = entity_stmt
        .query_map(param_refs.as_slice(), |row| {
            Ok(Entity {
                id: row.get(0)?,
                name: row.get(1)?,
                entity_type: row.get(2)?,
                scope: row.get(3)?,
                observations: Vec::new(),
                created_at: row.get(4)?,
                updated_at: row.get(5)?,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;

    let mut entities = Vec::with_capacity(entities_raw.len());
    for e in entities_raw {
        let observations = get_observations(conn, &e.id)?;
        entities.push(Entity { observations, ..e });
    }

    // Build scope filter for relations
    let mut rel_sql = String::from(
        "SELECT r.id, ef.name, r.from_entity, et.name, r.to_entity, r.relation_type, r.scope, r.created_at
         FROM relations r
         JOIN entities ef ON ef.id = r.from_entity
         JOIN entities et ON et.id = r.to_entity
         WHERE ",
    );
    let mut rel_params: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();
    let mut idx = 1;
    let mut clauses = Vec::with_capacity(scopes.len());
    for s in scopes {
        clauses.push(format!(
            "(r.scope = ?{idx} OR r.scope LIKE ?{idx2} ESCAPE '\\')",
            idx = idx,
            idx2 = idx + 1,
        ));
        rel_params.push(Box::new(s.to_string()));
        rel_params.push(Box::new(format!("{}/%", escape_like(s))));
        idx += 2;
    }
    rel_sql.push_str(&format!(
        "({}) ORDER BY ef.name, et.name",
        clauses.join(" OR ")
    ));

    let mut rel_stmt = conn.prepare(&rel_sql)?;
    let rel_param_refs: Vec<&dyn rusqlite::types::ToSql> =
        rel_params.iter().map(|p| p.as_ref()).collect();
    let relations: Vec<Relation> = rel_stmt
        .query_map(rel_param_refs.as_slice(), |row| {
            Ok(Relation {
                id: row.get(0)?,
                from_entity: row.get(1)?,
                from_entity_id: row.get(2)?,
                to_entity: row.get(3)?,
                to_entity_id: row.get(4)?,
                relation_type: row.get(5)?,
                scope: row.get(6)?,
                created_at: row.get(7)?,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;

    Ok(Graph {
        entities,
        relations,
    })
}

pub fn search_entities(
    conn: &Connection,
    query: &str,
    scopes: Option<&[String]>,
    limit: usize,
) -> rusqlite::Result<Vec<EntitySearchResult>> {
    // Search entity names/types via FTS, then observation content via FTS
    // Union the entity IDs and return full entities with observations
    let mut sql = String::from(
        "SELECT DISTINCT e.id, MIN(rank) as best_rank FROM (
            SELECT e.id, fts.rank
            FROM entities_fts fts
            JOIN entities e ON e.rowid = fts.rowid
            WHERE entities_fts MATCH ?1
            UNION ALL
            SELECT o.entity_id as id, fts.rank
            FROM observations_fts fts
            JOIN observations o ON o.rowid = fts.rowid
            WHERE observations_fts MATCH ?1
        ) sub
        JOIN entities e ON e.id = sub.id
        WHERE 1=1",
    );
    let mut sql_params: Vec<Box<dyn rusqlite::types::ToSql>> = vec![Box::new(query.to_string())];
    let mut param_idx = 2;

    if let Some(scopes) = scopes {
        if !scopes.is_empty() {
            let mut clauses = Vec::with_capacity(scopes.len());
            for s in scopes {
                clauses.push(format!(
                    "(e.scope = ?{idx} OR e.scope LIKE ?{idx2} ESCAPE '\\')",
                    idx = param_idx,
                    idx2 = param_idx + 1,
                ));
                sql_params.push(Box::new(s.to_string()));
                sql_params.push(Box::new(format!("{}/%", escape_like(s))));
                param_idx += 2;
            }
            sql.push_str(&format!(" AND ({})", clauses.join(" OR ")));
        }
    }

    sql.push_str(&format!(
        " GROUP BY e.id ORDER BY best_rank LIMIT ?{param_idx}"
    ));
    sql_params.push(Box::new(limit as i64));

    let mut stmt = conn.prepare(&sql)?;
    let param_refs: Vec<&dyn rusqlite::types::ToSql> =
        sql_params.iter().map(|p| p.as_ref()).collect();

    let rows: Vec<(String, Option<f64>)> = stmt
        .query_map(param_refs.as_slice(), |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, Option<f64>>(1)?))
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;

    let mut results = Vec::with_capacity(rows.len());
    for (entity_id, rank) in rows {
        if let Some(entity) = get_entity_by_id(conn, &entity_id)? {
            results.push(EntitySearchResult { entity, rank });
        }
    }
    Ok(results)
}

pub fn open_entities(
    conn: &Connection,
    names: &[String],
    scopes: &[String],
) -> rusqlite::Result<Graph> {
    let mut entities = Vec::new();
    let mut entity_ids = Vec::new();

    if !names.is_empty() && !scopes.is_empty() {
        // Build a query that finds entities matching any name + any scope (prefix-matched)
        let mut sql = String::from(
            "SELECT id, name, entity_type, scope, created_at, updated_at FROM entities WHERE ",
        );
        let mut sql_params: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();
        let mut idx = 1;

        // Name filter
        let name_placeholders: Vec<String> = names
            .iter()
            .enumerate()
            .map(|(i, _)| format!("?{}", idx + i))
            .collect();
        sql.push_str(&format!("name IN ({})", name_placeholders.join(", ")));
        for n in names {
            sql_params.push(Box::new(n.to_string()));
        }
        idx += names.len();

        // Scope filter (prefix-matched)
        let mut scope_clauses = Vec::with_capacity(scopes.len());
        for s in scopes {
            scope_clauses.push(format!(
                "(scope = ?{idx} OR scope LIKE ?{idx2} ESCAPE '\\')",
                idx = idx,
                idx2 = idx + 1,
            ));
            sql_params.push(Box::new(s.to_string()));
            sql_params.push(Box::new(format!("{}/%", escape_like(s))));
            idx += 2;
        }
        sql.push_str(&format!(" AND ({})", scope_clauses.join(" OR ")));

        let mut stmt = conn.prepare(&sql)?;
        let param_refs: Vec<&dyn rusqlite::types::ToSql> =
            sql_params.iter().map(|p| p.as_ref()).collect();
        let rows: Vec<Entity> = stmt
            .query_map(param_refs.as_slice(), |row| {
                Ok(Entity {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    entity_type: row.get(2)?,
                    scope: row.get(3)?,
                    observations: Vec::new(),
                    created_at: row.get(4)?,
                    updated_at: row.get(5)?,
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;

        for e in rows {
            let observations = get_observations(conn, &e.id)?;
            entity_ids.push(e.id.clone());
            entities.push(Entity { observations, ..e });
        }
    }

    if entity_ids.is_empty() {
        return Ok(Graph {
            entities: Vec::new(),
            relations: Vec::new(),
        });
    }

    // Get all relations involving these entities (from or to)
    let placeholders: Vec<String> = entity_ids
        .iter()
        .enumerate()
        .map(|(i, _)| format!("?{}", i + 1))
        .collect();
    let ph_str = placeholders.join(", ");
    let sql = format!(
        "SELECT r.id, ef.name, r.from_entity, et.name, r.to_entity, r.relation_type, r.scope, r.created_at
         FROM relations r
         JOIN entities ef ON ef.id = r.from_entity
         JOIN entities et ON et.id = r.to_entity
         WHERE r.from_entity IN ({ph_str}) OR r.to_entity IN ({ph_str})"
    );

    let mut stmt = conn.prepare(&sql)?;
    // Build params: each entity_id appears once in the param list, but is referenced twice
    let sql_params: Vec<Box<dyn rusqlite::types::ToSql>> = entity_ids
        .iter()
        .map(|id| Box::new(id.clone()) as Box<dyn rusqlite::types::ToSql>)
        .collect();
    let param_refs: Vec<&dyn rusqlite::types::ToSql> =
        sql_params.iter().map(|p| p.as_ref()).collect();

    let relations: Vec<Relation> = stmt
        .query_map(param_refs.as_slice(), |row| {
            Ok(Relation {
                id: row.get(0)?,
                from_entity: row.get(1)?,
                from_entity_id: row.get(2)?,
                to_entity: row.get(3)?,
                to_entity_id: row.get(4)?,
                relation_type: row.get(5)?,
                scope: row.get(6)?,
                created_at: row.get(7)?,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;

    // Also load neighbor entities that aren't already in the list
    let mut neighbor_ids: Vec<String> = Vec::new();
    for rel in &relations {
        if !entity_ids.contains(&rel.from_entity_id) && !neighbor_ids.contains(&rel.from_entity_id)
        {
            neighbor_ids.push(rel.from_entity_id.clone());
        }
        if !entity_ids.contains(&rel.to_entity_id) && !neighbor_ids.contains(&rel.to_entity_id) {
            neighbor_ids.push(rel.to_entity_id.clone());
        }
    }
    for nid in &neighbor_ids {
        if let Some(entity) = get_entity_by_id(conn, nid)? {
            entities.push(entity);
        }
    }

    Ok(Graph {
        entities,
        relations,
    })
}

// ── Mailbox (postal, PRD-027) ────────────────────────────────────────────────

#[derive(Debug, Clone, serde::Serialize)]
pub struct MailboxRow {
    pub message_id: i64,
    pub mail_uid: String,
    pub to_project: String,
    pub attention: Option<String>,
    #[serde(rename = "from")]
    pub from_addr: String,
    pub subject: String,
    pub message_type: String,
    pub meta: serde_json::Value,
    pub payload: serde_json::Value,
    pub created_at: String,
    pub status: String,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct OutboxRow {
    pub mail_uid: String,
    pub to_host: String,
    pub to_project: String,
    pub attention: Option<String>,
    #[serde(rename = "from")]
    pub from_addr: String,
    pub subject: String,
    pub message_type: String,
    pub meta: serde_json::Value,
    pub payload: serde_json::Value,
    pub status: String,
    pub attempts: i64,
    pub next_attempt_at: String,
    pub last_error: Option<String>,
    pub remote_message_id: Option<i64>,
    pub created_at: String,
    pub delivered_at: Option<String>,
}

fn steop_now() -> String {
    chrono::Utc::now().to_rfc3339()
}

fn steop_parse_json(s: &str) -> serde_json::Value {
    serde_json::from_str(s).unwrap_or_else(|_| serde_json::Value::Object(Default::default()))
}

fn steop_json_merge(base: &mut serde_json::Value, patch: serde_json::Value) {
    if let (serde_json::Value::Object(b), serde_json::Value::Object(p)) = (base, patch) {
        for (k, v) in p {
            b.insert(k, v);
        }
    }
}

// ── Mailbox ──────────────────────────────────────────────────────────────────

/// Outcome of `steop_mailbox_read` / `steop_mailbox_archive`. The `Conflict`
/// variant carries the current status so the handler can return 409 with the
/// server's actual state.
#[derive(Debug)]
pub enum MailboxTransition {
    Ok,
    NotFound,
    Conflict(String),
}

const INBOX_COLS: &str = "message_id, mail_uid, to_project, attention, from_addr, \
     subject, message_type, meta, payload, created_at, status";

/// Insert a delivered/local mail row, deduping on `mail_uid`.
/// Returns `(message_id, "stored"|"duplicate")`.
#[allow(clippy::too_many_arguments)]
pub fn mailbox_inbox_upsert(
    conn: &Connection,
    mail_uid: &str,
    to_project: &str,
    attention: Option<&str>,
    from_addr: &str,
    subject: &str,
    message_type: &str,
    meta: &serde_json::Value,
    payload: &serde_json::Value,
    status: &str,
) -> rusqlite::Result<(i64, String)> {
    let now = steop_now();
    let inserted = conn.execute(
        "INSERT INTO mailbox_inbox
         (mail_uid, to_project, attention, from_addr, subject, message_type, meta, payload, created_at, status)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
         ON CONFLICT(mail_uid) DO NOTHING",
        rusqlite::params![
            mail_uid,
            to_project,
            attention,
            from_addr,
            subject,
            message_type,
            meta.to_string(),
            payload.to_string(),
            now,
            status,
        ],
    )?;
    let message_id: i64 = conn.query_row(
        "SELECT message_id FROM mailbox_inbox WHERE mail_uid = ?1",
        rusqlite::params![mail_uid],
        |r| r.get(0),
    )?;
    let outcome = if inserted == 1 { "stored" } else { "duplicate" };
    Ok((message_id, outcome.to_string()))
}

pub fn mailbox_inbox_get(
    conn: &Connection,
    message_id: i64,
) -> rusqlite::Result<Option<MailboxRow>> {
    use rusqlite::OptionalExtension;
    OptionalExtension::optional(conn.query_row(
        &format!("SELECT {} FROM mailbox_inbox WHERE message_id = ?1", INBOX_COLS),
        rusqlite::params![message_id],
        mailbox_row_from,
    ))
}

/// List inbox rows for `to_project` visible to the caller's alias set.
/// A row is visible when `attention IS NULL` (household), `attention = '*'`
/// (broadcast), or `lower(attention)` is one of `aliases` (already lowercased
/// by the caller). `statuses` is non-empty (handler defaults).
pub fn mailbox_inbox_list(
    conn: &Connection,
    to_project: &str,
    aliases: &[String],
    statuses: &[String],
) -> rusqlite::Result<Vec<MailboxRow>> {
    let mut args: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();
    args.push(Box::new(to_project.to_string()));

    // attention clause
    let mut attention_clause = String::from("(attention IS NULL OR attention = '*'");
    if !aliases.is_empty() {
        let mut placeholders: Vec<String> = Vec::new();
        for a in aliases {
            args.push(Box::new(a.to_lowercase()));
            placeholders.push(format!("?{}", args.len()));
        }
        attention_clause.push_str(&format!(
            " OR lower(attention) IN ({})",
            placeholders.join(",")
        ));
    }
    attention_clause.push(')');

    // status clause
    let mut status_placeholders: Vec<String> = Vec::new();
    for s in statuses {
        args.push(Box::new(s.clone()));
        status_placeholders.push(format!("?{}", args.len()));
    }
    let status_clause = format!("status IN ({})", status_placeholders.join(","));

    let sql = format!(
        "SELECT {} FROM mailbox_inbox \
         WHERE to_project = ?1 AND {} AND {} \
         ORDER BY created_at ASC LIMIT 1000",
        INBOX_COLS, attention_clause, status_clause
    );

    let mut stmt = conn.prepare(&sql)?;
    let param_refs: Vec<&dyn rusqlite::ToSql> =
        args.iter().map(|b| b.as_ref() as &dyn rusqlite::ToSql).collect();
    let rows = stmt
        .query_map(param_refs.as_slice(), mailbox_row_from)?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

pub fn mailbox_inbox_read(
    conn: &Connection,
    message_id: i64,
) -> rusqlite::Result<MailboxTransition> {
    use rusqlite::OptionalExtension;
    let current: Option<String> = OptionalExtension::optional(conn.query_row(
        "SELECT status FROM mailbox_inbox WHERE message_id = ?1",
        rusqlite::params![message_id],
        |r| r.get(0),
    ))?;
    let current = match current {
        None => return Ok(MailboxTransition::NotFound),
        Some(s) => s,
    };
    if current != "NEW" {
        return Ok(MailboxTransition::Conflict(current));
    }
    conn.execute(
        "UPDATE mailbox_inbox SET status='READ' WHERE message_id = ?1",
        rusqlite::params![message_id],
    )?;
    Ok(MailboxTransition::Ok)
}

pub fn mailbox_inbox_archive(
    conn: &Connection,
    message_id: i64,
) -> rusqlite::Result<MailboxTransition> {
    use rusqlite::OptionalExtension;
    let current: Option<String> = OptionalExtension::optional(conn.query_row(
        "SELECT status FROM mailbox_inbox WHERE message_id = ?1",
        rusqlite::params![message_id],
        |r| r.get(0),
    ))?;
    let current = match current {
        None => return Ok(MailboxTransition::NotFound),
        Some(s) => s,
    };
    if current == "ARCHIVE" {
        return Ok(MailboxTransition::Conflict(current));
    }
    conn.execute(
        "UPDATE mailbox_inbox SET status='ARCHIVE' WHERE message_id = ?1",
        rusqlite::params![message_id],
    )?;
    Ok(MailboxTransition::Ok)
}

#[allow(dead_code)]
pub fn mailbox_inbox_update_meta(
    conn: &mut Connection,
    message_id: i64,
    meta_patch: serde_json::Value,
) -> rusqlite::Result<Option<MailboxRow>> {
    use rusqlite::OptionalExtension;
    let tx = conn.transaction()?;
    let current_meta: Option<String> = OptionalExtension::optional(tx.query_row(
        "SELECT meta FROM mailbox_inbox WHERE message_id = ?1",
        rusqlite::params![message_id],
        |r| r.get(0),
    ))?;
    let current_meta_s = match current_meta {
        None => return Ok(None),
        Some(s) => s,
    };
    let mut merged = steop_parse_json(&current_meta_s);
    steop_json_merge(&mut merged, meta_patch);
    let merged_s = merged.to_string();
    tx.execute(
        "UPDATE mailbox_inbox SET meta = ?1 WHERE message_id = ?2",
        rusqlite::params![merged_s, message_id],
    )?;
    let row = tx.query_row(
        &format!("SELECT {} FROM mailbox_inbox WHERE message_id = ?1", INBOX_COLS),
        rusqlite::params![message_id],
        mailbox_row_from,
    )?;
    tx.commit()?;
    Ok(Some(row))
}

fn mailbox_row_from(row: &rusqlite::Row) -> rusqlite::Result<MailboxRow> {
    let meta_s: String = row.get(7)?;
    let payload_s: String = row.get(8)?;
    Ok(MailboxRow {
        message_id: row.get(0)?,
        mail_uid: row.get(1)?,
        to_project: row.get(2)?,
        attention: row.get(3)?,
        from_addr: row.get(4)?,
        subject: row.get(5)?,
        message_type: row.get(6)?,
        meta: steop_parse_json(&meta_s),
        payload: steop_parse_json(&payload_s),
        created_at: row.get(9)?,
        status: row.get(10)?,
    })
}

// ── Outbox (origin spool) ─────────────────────────────────────────────────────

const OUTBOX_COLS: &str = "mail_uid, to_host, to_project, attention, from_addr, \
     subject, message_type, meta, payload, status, attempts, next_attempt_at, \
     last_error, remote_message_id, created_at, delivered_at";

#[allow(clippy::too_many_arguments)]
pub fn mailbox_outbox_enqueue(
    conn: &Connection,
    mail_uid: &str,
    to_host: &str,
    to_project: &str,
    attention: Option<&str>,
    from_addr: &str,
    subject: &str,
    message_type: &str,
    meta: &serde_json::Value,
    payload: &serde_json::Value,
) -> rusqlite::Result<()> {
    let now = steop_now();
    conn.execute(
        "INSERT INTO mailbox_outbox
         (mail_uid, to_host, to_project, attention, from_addr, subject, message_type,
          meta, payload, status, attempts, next_attempt_at, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, 'QUEUED', 0, ?10, ?10)
         ON CONFLICT(mail_uid) DO NOTHING",
        rusqlite::params![
            mail_uid,
            to_host,
            to_project,
            attention,
            from_addr,
            subject,
            message_type,
            meta.to_string(),
            payload.to_string(),
            now,
        ],
    )?;
    Ok(())
}

/// Return `QUEUED` rows whose `next_attempt_at <= now`, oldest first.
pub fn mailbox_outbox_due(conn: &Connection, now: &str) -> rusqlite::Result<Vec<OutboxRow>> {
    let mut stmt = conn.prepare(&format!(
        "SELECT {} FROM mailbox_outbox \
         WHERE status = 'QUEUED' AND next_attempt_at <= ?1 \
         ORDER BY next_attempt_at ASC",
        OUTBOX_COLS
    ))?;
    let rows = stmt
        .query_map(rusqlite::params![now], outbox_row_from)?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

pub fn mailbox_outbox_list(
    conn: &Connection,
    statuses: &[String],
) -> rusqlite::Result<Vec<OutboxRow>> {
    let placeholders: Vec<String> = (0..statuses.len())
        .map(|i| format!("?{}", i + 1))
        .collect();
    let sql = format!(
        "SELECT {} FROM mailbox_outbox WHERE status IN ({}) ORDER BY created_at ASC LIMIT 1000",
        OUTBOX_COLS,
        placeholders.join(",")
    );
    let mut stmt = conn.prepare(&sql)?;
    let args: Vec<&dyn rusqlite::ToSql> =
        statuses.iter().map(|s| s as &dyn rusqlite::ToSql).collect();
    let rows = stmt
        .query_map(args.as_slice(), outbox_row_from)?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

/// Update outbox lifecycle fields after a delivery attempt.
pub fn mailbox_outbox_mark(
    conn: &Connection,
    mail_uid: &str,
    status: &str,
    attempts: i64,
    next_attempt_at: &str,
    last_error: Option<&str>,
    remote_message_id: Option<i64>,
    delivered_at: Option<&str>,
) -> rusqlite::Result<()> {
    conn.execute(
        "UPDATE mailbox_outbox
         SET status = ?2, attempts = ?3, next_attempt_at = ?4, last_error = ?5,
             remote_message_id = ?6, delivered_at = ?7
         WHERE mail_uid = ?1",
        rusqlite::params![
            mail_uid,
            status,
            attempts,
            next_attempt_at,
            last_error,
            remote_message_id,
            delivered_at,
        ],
    )?;
    Ok(())
}

fn outbox_row_from(row: &rusqlite::Row) -> rusqlite::Result<OutboxRow> {
    let meta_s: String = row.get(7)?;
    let payload_s: String = row.get(8)?;
    Ok(OutboxRow {
        mail_uid: row.get(0)?,
        to_host: row.get(1)?,
        to_project: row.get(2)?,
        attention: row.get(3)?,
        from_addr: row.get(4)?,
        subject: row.get(5)?,
        message_type: row.get(6)?,
        meta: steop_parse_json(&meta_s),
        payload: steop_parse_json(&payload_s),
        status: row.get(9)?,
        attempts: row.get(10)?,
        next_attempt_at: row.get(11)?,
        last_error: row.get(12)?,
        remote_message_id: row.get(13)?,
        created_at: row.get(14)?,
        delivered_at: row.get(15)?,
    })
}

// ── Agent alias registry ──────────────────────────────────────────────────────

/// Upsert caller aliases for `project_dir`. Aliases are lowercased; the reserved
/// `*` alias is rejected (skipped). Returns the accepted (lowercased) aliases.
pub fn mailbox_alias_upsert(
    conn: &Connection,
    project_dir: &str,
    aliases: &[String],
) -> rusqlite::Result<Vec<String>> {
    let now = steop_now();
    let mut accepted = Vec::new();
    for a in aliases {
        let lower = a.trim().to_lowercase();
        if lower.is_empty() || lower == "*" {
            continue;
        }
        conn.execute(
            "INSERT INTO mailbox_agent_alias (project_dir, alias, last_seen)
             VALUES (?1, ?2, ?3)
             ON CONFLICT(project_dir, alias) DO UPDATE SET last_seen = excluded.last_seen",
            rusqlite::params![project_dir, lower, now],
        )?;
        accepted.push(lower);
    }
    Ok(accepted)
}

pub fn mailbox_alias_list(conn: &Connection, project_dir: &str) -> rusqlite::Result<Vec<String>> {
    let mut stmt = conn.prepare(
        "SELECT alias FROM mailbox_agent_alias WHERE project_dir = ?1 ORDER BY alias ASC",
    )?;
    let rows = stmt
        .query_map(rusqlite::params![project_dir], |r| r.get::<_, String>(0))?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}
