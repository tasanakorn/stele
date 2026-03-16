use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum MemoryType {
    Knowledge,
    Decision,
    Convention,
    Troubleshooting,
    Reference,
    Other,
}

impl MemoryType {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Knowledge => "knowledge",
            Self::Decision => "decision",
            Self::Convention => "convention",
            Self::Troubleshooting => "troubleshooting",
            Self::Reference => "reference",
            Self::Other => "other",
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s {
            "knowledge" => Self::Knowledge,
            "decision" => Self::Decision,
            "convention" => Self::Convention,
            "troubleshooting" => Self::Troubleshooting,
            "reference" => Self::Reference,
            _ => Self::Other,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Memory {
    pub id: String,
    pub title: String,
    pub content: String,
    pub memory_type: MemoryType,
    pub scope: String,
    pub author: Option<String>,
    pub tags: Vec<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResult {
    pub id: String,
    pub title: String,
    pub content: String,
    pub memory_type: MemoryType,
    pub scope: String,
    pub author: Option<String>,
    pub tags: Vec<String>,
    pub created_at: String,
    pub updated_at: String,
    pub rank: Option<f64>,
}

impl From<Memory> for SearchResult {
    fn from(m: Memory) -> Self {
        Self {
            id: m.id,
            title: m.title,
            content: m.content,
            memory_type: m.memory_type,
            scope: m.scope,
            author: m.author,
            tags: m.tags,
            created_at: m.created_at,
            updated_at: m.updated_at,
            rank: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScopeInfo {
    pub scope: String,
    pub count: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TagInfo {
    pub tag: String,
    pub count: i64,
}

#[derive(Debug, Clone)]
pub struct Stats {
    pub total_memories: i64,
    pub total_scopes: i64,
    pub total_tags: i64,
    pub recent_memories: Vec<RecentMemorySummary>,
}

#[derive(Debug, Clone)]
pub struct RecentMemorySummary {
    pub id: String,
    pub title: String,
    pub scope: String,
    pub updated_at: String,
}

// ── Knowledge Graph types ──

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Entity {
    pub id: String,
    pub name: String,
    pub entity_type: String,
    pub scope: String,
    pub observations: Vec<Observation>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Observation {
    pub id: String,
    pub content: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Relation {
    pub id: String,
    pub from_entity: String,
    pub from_entity_id: String,
    pub to_entity: String,
    pub to_entity_id: String,
    pub relation_type: String,
    pub scope: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Graph {
    pub entities: Vec<Entity>,
    pub relations: Vec<Relation>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntitySearchResult {
    pub entity: Entity,
    pub rank: Option<f64>,
}
