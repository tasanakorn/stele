use crate::client::SteleClient;
use crate::output::{print_json, print_memories, print_memory};

#[allow(clippy::too_many_arguments)]
pub fn handle_store(
    client: &SteleClient,
    title: &str,
    content: &str,
    scope: &str,
    tags: Option<Vec<String>>,
    memory_type: Option<String>,
    author: Option<String>,
    json: bool,
) {
    match client.store_memory(title, content, scope, tags, memory_type, author) {
        Ok(memory) => {
            if json {
                let v = serde_json::to_value(&memory).unwrap_or_default();
                print_json(&v);
            } else {
                println!("Memory stored.");
                print_memory(&memory);
            }
        }
        Err(e) => {
            eprintln!("Error: {}", e);
            std::process::exit(1);
        }
    }
}

pub fn handle_recall(
    client: &SteleClient,
    query: Option<&str>,
    scope: Option<&str>,
    tags: Option<Vec<String>>,
    match_all_tags: bool,
    limit: Option<u32>,
    json: bool,
) {
    match client.recall_memories(query, scope, tags, match_all_tags, limit) {
        Ok(results) => {
            if json {
                let v = serde_json::to_value(&results).unwrap_or_default();
                print_json(&v);
            } else {
                print_memories(&results);
            }
        }
        Err(e) => {
            eprintln!("Error: {}", e);
            std::process::exit(1);
        }
    }
}

pub fn handle_get(client: &SteleClient, id: &str, json: bool) {
    match client.get_memory(id) {
        Ok(memory) => {
            if json {
                let v = serde_json::to_value(&memory).unwrap_or_default();
                print_json(&v);
            } else {
                print_memory(&memory);
            }
        }
        Err(e) => {
            eprintln!("Error: {}", e);
            std::process::exit(1);
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub fn handle_update(
    client: &SteleClient,
    id: &str,
    title: Option<String>,
    content: Option<String>,
    scope: Option<String>,
    tags: Option<Vec<String>>,
    memory_type: Option<String>,
    json: bool,
) {
    match client.update_memory(id, title, content, scope, tags, memory_type) {
        Ok(memory) => {
            if json {
                let v = serde_json::to_value(&memory).unwrap_or_default();
                print_json(&v);
            } else {
                println!("Memory updated.");
                print_memory(&memory);
            }
        }
        Err(e) => {
            eprintln!("Error: {}", e);
            std::process::exit(1);
        }
    }
}

pub fn handle_forget(client: &SteleClient, id: &str, json: bool) {
    match client.delete_memory(id) {
        Ok(v) => {
            if json {
                print_json(&v);
            } else {
                println!("Memory {} deleted.", id);
            }
        }
        Err(e) => {
            eprintln!("Error: {}", e);
            std::process::exit(1);
        }
    }
}
