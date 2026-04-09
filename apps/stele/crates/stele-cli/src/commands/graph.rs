use crate::client::SteleClient;
use crate::output::{print_graph, print_json};

pub fn handle_graph_read(client: &SteleClient, scope: &str, json: bool) {
    match client.graph_read(scope) {
        Ok(graph) => {
            if json {
                let v = serde_json::to_value(&graph).unwrap_or_default();
                print_json(&v);
            } else {
                print_graph(&graph);
            }
        }
        Err(e) => {
            eprintln!("Error: {}", e);
            std::process::exit(1);
        }
    }
}

pub fn handle_graph_search(
    client: &SteleClient,
    query: &str,
    scope: Option<&str>,
    limit: Option<u32>,
    json: bool,
) {
    match client.graph_search(query, scope, limit) {
        Ok(results) => {
            if json {
                let v = serde_json::to_value(&results).unwrap_or_default();
                print_json(&v);
            } else if results.is_empty() {
                println!("No entities found.");
            } else {
                for r in &results {
                    println!(
                        "[{}] {} ({})",
                        r.entity.entity_type, r.entity.name, r.entity.scope
                    );
                    for obs in &r.entity.observations {
                        println!("  - {}", obs.content);
                    }
                }
            }
        }
        Err(e) => {
            eprintln!("Error: {}", e);
            std::process::exit(1);
        }
    }
}

pub fn handle_graph_open(client: &SteleClient, names: &str, scope: Option<&str>, json: bool) {
    match client.graph_open(names, scope) {
        Ok(graph) => {
            if json {
                let v = serde_json::to_value(&graph).unwrap_or_default();
                print_json(&v);
            } else {
                print_graph(&graph);
            }
        }
        Err(e) => {
            eprintln!("Error: {}", e);
            std::process::exit(1);
        }
    }
}

pub fn handle_entities_create(
    client: &SteleClient,
    name: &str,
    entity_type: &str,
    scope: &str,
    observations: Option<Vec<String>>,
    json: bool,
) {
    let entity_json = serde_json::json!([{
        "name": name,
        "entity_type": entity_type,
        "observations": observations.unwrap_or_default(),
    }]);
    match client.graph_create_entities(entity_json, scope) {
        Ok(v) => {
            if json {
                print_json(&v);
            } else {
                println!("Entity created: {} ({})", name, entity_type);
            }
        }
        Err(e) => {
            eprintln!("Error: {}", e);
            std::process::exit(1);
        }
    }
}

pub fn handle_entities_get(client: &SteleClient, name: &str, scope: &str, json: bool) {
    match client.graph_get_entity(name, scope) {
        Ok(entity) => {
            if json {
                let v = serde_json::to_value(&entity).unwrap_or_default();
                print_json(&v);
            } else {
                println!(
                    "[{}] {} ({})",
                    entity.entity_type, entity.name, entity.scope
                );
                for obs in &entity.observations {
                    println!("  - {}", obs.content);
                }
            }
        }
        Err(e) => {
            eprintln!("Error: {}", e);
            std::process::exit(1);
        }
    }
}

pub fn handle_entities_delete(client: &SteleClient, name: &str, scope: &str, json: bool) {
    match client.graph_delete_entity(name, scope) {
        Ok(v) => {
            if json {
                print_json(&v);
            } else {
                println!("Entity deleted: {}", name);
            }
        }
        Err(e) => {
            eprintln!("Error: {}", e);
            std::process::exit(1);
        }
    }
}

pub fn handle_observations_add(
    client: &SteleClient,
    entity_name: &str,
    scope: &str,
    observations: Vec<String>,
    json: bool,
) {
    match client.graph_add_observations(entity_name, scope, observations) {
        Ok(entity) => {
            if json {
                let v = serde_json::to_value(&entity).unwrap_or_default();
                print_json(&v);
            } else {
                println!("Observations added to: {}", entity_name);
                for obs in &entity.observations {
                    println!("  - {}", obs.content);
                }
            }
        }
        Err(e) => {
            eprintln!("Error: {}", e);
            std::process::exit(1);
        }
    }
}

pub fn handle_observations_delete(
    client: &SteleClient,
    entity_name: &str,
    scope: &str,
    observations: Vec<String>,
    json: bool,
) {
    match client.graph_delete_observations(entity_name, scope, observations) {
        Ok(entity) => {
            if json {
                let v = serde_json::to_value(&entity).unwrap_or_default();
                print_json(&v);
            } else {
                println!("Observations deleted from: {}", entity_name);
            }
        }
        Err(e) => {
            eprintln!("Error: {}", e);
            std::process::exit(1);
        }
    }
}

pub fn handle_relations_create(
    client: &SteleClient,
    from: &str,
    to: &str,
    relation_type: &str,
    scope: &str,
    json: bool,
) {
    let relations_json = serde_json::json!([{
        "from_entity": from,
        "to_entity": to,
        "relation_type": relation_type,
    }]);
    match client.graph_create_relations(relations_json, scope) {
        Ok(v) => {
            if json {
                print_json(&v);
            } else {
                println!("Relation created: {} --[{}]--> {}", from, relation_type, to);
            }
        }
        Err(e) => {
            eprintln!("Error: {}", e);
            std::process::exit(1);
        }
    }
}

pub fn handle_relations_delete(
    client: &SteleClient,
    from: &str,
    to: &str,
    relation_type: &str,
    scope: &str,
    json: bool,
) {
    let relations_json = serde_json::json!([{
        "from_entity": from,
        "to_entity": to,
        "relation_type": relation_type,
    }]);
    match client.graph_delete_relations(relations_json, scope) {
        Ok(v) => {
            if json {
                print_json(&v);
            } else {
                println!("Relation deleted: {} --[{}]--> {}", from, relation_type, to);
            }
        }
        Err(e) => {
            eprintln!("Error: {}", e);
            std::process::exit(1);
        }
    }
}
