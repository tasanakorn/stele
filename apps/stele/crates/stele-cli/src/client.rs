use serde_json::Value;
use stele_common::models::{
    Entity, EntitySearchResult, Graph, Memory, ScopeInfo, SearchResult, TagInfo,
};
use ureq::Agent;

#[derive(Debug)]
pub enum ClientError {
    Http(String),
    Json(String),
}

impl std::fmt::Display for ClientError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ClientError::Http(s) => write!(f, "HTTP error: {s}"),
            ClientError::Json(s) => write!(f, "JSON error: {s}"),
        }
    }
}

pub struct SteleClient {
    agent: Agent,
    base_url: String,
    auth_key: Option<String>,
}

impl SteleClient {
    pub fn new(base_url: String, auth_key: Option<String>) -> Self {
        Self {
            agent: Agent::new(),
            base_url,
            auth_key,
        }
    }

    fn get(&self, path: &str) -> ureq::Request {
        let req = self.agent.get(&format!("{}{}", self.base_url, path));
        if let Some(ref key) = self.auth_key {
            req.set("X-Stele-Key", key)
        } else {
            req
        }
    }

    fn post(&self, path: &str) -> ureq::Request {
        let req = self.agent.post(&format!("{}{}", self.base_url, path));
        if let Some(ref key) = self.auth_key {
            req.set("X-Stele-Key", key)
        } else {
            req
        }
    }

    fn put(&self, path: &str) -> ureq::Request {
        let req = self.agent.put(&format!("{}{}", self.base_url, path));
        if let Some(ref key) = self.auth_key {
            req.set("X-Stele-Key", key)
        } else {
            req
        }
    }

    fn delete(&self, path: &str) -> ureq::Request {
        let req = self.agent.delete(&format!("{}{}", self.base_url, path));
        if let Some(ref key) = self.auth_key {
            req.set("X-Stele-Key", key)
        } else {
            req
        }
    }

    pub fn store_memory(
        &self,
        title: &str,
        content: &str,
        scope: &str,
        tags: Option<Vec<String>>,
        memory_type: Option<String>,
        author: Option<String>,
    ) -> Result<Memory, ClientError> {
        let mut body = serde_json::json!({
            "title": title,
            "content": content,
            "scope": scope,
        });
        if let Some(t) = tags {
            body["tags"] = serde_json::json!(t);
        }
        if let Some(mt) = memory_type {
            body["memory_type"] = serde_json::json!(mt);
        }
        if let Some(a) = author {
            body["author"] = serde_json::json!(a);
        }
        let resp = self
            .post("/api/v1/memories")
            .send_json(body)
            .map_err(|e| ClientError::Http(e.to_string()))?;
        resp.into_json::<Memory>()
            .map_err(|e| ClientError::Json(e.to_string()))
    }

    pub fn recall_memories(
        &self,
        query: Option<&str>,
        scope: Option<&str>,
        tags: Option<Vec<String>>,
        match_all_tags: bool,
        limit: Option<u32>,
    ) -> Result<Vec<SearchResult>, ClientError> {
        let mut req = self.get("/api/v1/memories");
        if let Some(q) = query {
            req = req.query("q", q);
        }
        if let Some(s) = scope {
            req = req.query("scope", s);
        }
        if let Some(t) = tags {
            for tag in &t {
                req = req.query("tags", tag);
            }
        }
        if match_all_tags {
            req = req.query("match_all_tags", "true");
        }
        if let Some(l) = limit {
            req = req.query("limit", &l.to_string());
        }
        let resp = req.call().map_err(|e| ClientError::Http(e.to_string()))?;
        resp.into_json::<Vec<SearchResult>>()
            .map_err(|e| ClientError::Json(e.to_string()))
    }

    pub fn get_memory(&self, id: &str) -> Result<Memory, ClientError> {
        let resp = self
            .get(&format!("/api/v1/memories/{}", id))
            .call()
            .map_err(|e| ClientError::Http(e.to_string()))?;
        resp.into_json::<Memory>()
            .map_err(|e| ClientError::Json(e.to_string()))
    }

    pub fn update_memory(
        &self,
        id: &str,
        title: Option<String>,
        content: Option<String>,
        scope: Option<String>,
        tags: Option<Vec<String>>,
        memory_type: Option<String>,
    ) -> Result<Memory, ClientError> {
        let mut body = serde_json::json!({});
        if let Some(t) = title {
            body["title"] = serde_json::json!(t);
        }
        if let Some(c) = content {
            body["content"] = serde_json::json!(c);
        }
        if let Some(s) = scope {
            body["scope"] = serde_json::json!(s);
        }
        if let Some(t) = tags {
            body["tags"] = serde_json::json!(t);
        }
        if let Some(mt) = memory_type {
            body["memory_type"] = serde_json::json!(mt);
        }
        let resp = self
            .put(&format!("/api/v1/memories/{}", id))
            .send_json(body)
            .map_err(|e| ClientError::Http(e.to_string()))?;
        resp.into_json::<Memory>()
            .map_err(|e| ClientError::Json(e.to_string()))
    }

    pub fn delete_memory(&self, id: &str) -> Result<Value, ClientError> {
        let resp = self
            .delete(&format!("/api/v1/memories/{}", id))
            .call()
            .map_err(|e| ClientError::Http(e.to_string()))?;
        resp.into_json::<Value>()
            .map_err(|e| ClientError::Json(e.to_string()))
    }

    pub fn list_scopes(&self, prefix: Option<&str>) -> Result<Vec<ScopeInfo>, ClientError> {
        let mut req = self.get("/api/v1/scopes");
        if let Some(p) = prefix {
            req = req.query("prefix", p);
        }
        let resp = req.call().map_err(|e| ClientError::Http(e.to_string()))?;
        resp.into_json::<Vec<ScopeInfo>>()
            .map_err(|e| ClientError::Json(e.to_string()))
    }

    pub fn list_tags(&self, scope: Option<&str>) -> Result<Vec<TagInfo>, ClientError> {
        let mut req = self.get("/api/v1/tags");
        if let Some(s) = scope {
            req = req.query("scope", s);
        }
        let resp = req.call().map_err(|e| ClientError::Http(e.to_string()))?;
        resp.into_json::<Vec<TagInfo>>()
            .map_err(|e| ClientError::Json(e.to_string()))
    }

    pub fn get_stats(&self) -> Result<Value, ClientError> {
        let resp = self
            .get("/api/v1/stats")
            .call()
            .map_err(|e| ClientError::Http(e.to_string()))?;
        resp.into_json::<Value>()
            .map_err(|e| ClientError::Json(e.to_string()))
    }

    pub fn health_check(&self) -> bool {
        self.get("/api/v1/stats").call().is_ok()
    }

    pub fn graph_read(&self, scope: &str) -> Result<Graph, ClientError> {
        let resp = self
            .get("/api/v1/graph")
            .query("scope", scope)
            .call()
            .map_err(|e| ClientError::Http(e.to_string()))?;
        resp.into_json::<Graph>()
            .map_err(|e| ClientError::Json(e.to_string()))
    }

    pub fn graph_search(
        &self,
        query: &str,
        scope: Option<&str>,
        limit: Option<u32>,
    ) -> Result<Vec<EntitySearchResult>, ClientError> {
        let mut req = self.get("/api/v1/graph/entities").query("q", query);
        if let Some(s) = scope {
            req = req.query("scope", s);
        }
        if let Some(l) = limit {
            req = req.query("limit", &l.to_string());
        }
        let resp = req.call().map_err(|e| ClientError::Http(e.to_string()))?;
        resp.into_json::<Vec<EntitySearchResult>>()
            .map_err(|e| ClientError::Json(e.to_string()))
    }

    pub fn graph_open(&self, names: &str, scope: Option<&str>) -> Result<Graph, ClientError> {
        let mut req = self.get("/api/v1/graph/open").query("names", names);
        if let Some(s) = scope {
            req = req.query("scope", s);
        }
        let resp = req.call().map_err(|e| ClientError::Http(e.to_string()))?;
        resp.into_json::<Graph>()
            .map_err(|e| ClientError::Json(e.to_string()))
    }

    pub fn graph_create_entities(
        &self,
        entities_json: Value,
        scope: &str,
    ) -> Result<Value, ClientError> {
        let body = serde_json::json!({ "entities": entities_json, "scope": scope });
        let resp = self
            .post("/api/v1/graph/entities")
            .send_json(body)
            .map_err(|e| ClientError::Http(e.to_string()))?;
        resp.into_json::<Value>()
            .map_err(|e| ClientError::Json(e.to_string()))
    }

    pub fn graph_get_entity(&self, name: &str, scope: &str) -> Result<Entity, ClientError> {
        let resp = self
            .get(&format!("/api/v1/graph/entities/{}", name))
            .query("scope", scope)
            .call()
            .map_err(|e| ClientError::Http(e.to_string()))?;
        resp.into_json::<Entity>()
            .map_err(|e| ClientError::Json(e.to_string()))
    }

    pub fn graph_delete_entity(&self, name: &str, scope: &str) -> Result<Value, ClientError> {
        let resp = self
            .delete(&format!("/api/v1/graph/entities/{}", name))
            .query("scope", scope)
            .call()
            .map_err(|e| ClientError::Http(e.to_string()))?;
        resp.into_json::<Value>()
            .map_err(|e| ClientError::Json(e.to_string()))
    }

    pub fn graph_add_observations(
        &self,
        name: &str,
        scope: &str,
        observations: Vec<String>,
    ) -> Result<Entity, ClientError> {
        let body = serde_json::json!({ "observations": observations });
        let resp = self
            .post(&format!("/api/v1/graph/entities/{}/observations", name))
            .query("scope", scope)
            .send_json(body)
            .map_err(|e| ClientError::Http(e.to_string()))?;
        resp.into_json::<Entity>()
            .map_err(|e| ClientError::Json(e.to_string()))
    }

    pub fn graph_delete_observations(
        &self,
        name: &str,
        scope: &str,
        observations: Vec<String>,
    ) -> Result<Entity, ClientError> {
        let body = serde_json::json!({ "observations": observations });
        let resp = self
            .delete(&format!("/api/v1/graph/entities/{}/observations", name))
            .query("scope", scope)
            .send_json(body)
            .map_err(|e| ClientError::Http(e.to_string()))?;
        resp.into_json::<Entity>()
            .map_err(|e| ClientError::Json(e.to_string()))
    }

    pub fn graph_create_relations(
        &self,
        relations_json: Value,
        scope: &str,
    ) -> Result<Value, ClientError> {
        let body = serde_json::json!({ "relations": relations_json, "scope": scope });
        let resp = self
            .post("/api/v1/graph/relations")
            .send_json(body)
            .map_err(|e| ClientError::Http(e.to_string()))?;
        resp.into_json::<Value>()
            .map_err(|e| ClientError::Json(e.to_string()))
    }

    pub fn graph_delete_relations(
        &self,
        relations_json: Value,
        scope: &str,
    ) -> Result<Value, ClientError> {
        let body = serde_json::json!({ "relations": relations_json, "scope": scope });
        let resp = self
            .delete("/api/v1/graph/relations")
            .send_json(body)
            .map_err(|e| ClientError::Http(e.to_string()))?;
        resp.into_json::<Value>()
            .map_err(|e| ClientError::Json(e.to_string()))
    }
}
