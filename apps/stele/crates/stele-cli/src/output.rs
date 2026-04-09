use stele_common::models::{Graph, Memory, SearchResult};

pub fn print_json(value: &serde_json::Value) {
    println!(
        "{}",
        serde_json::to_string_pretty(value).unwrap_or_default()
    );
}

pub fn print_memory(memory: &Memory) {
    println!("ID:      {}", memory.id);
    println!("Title:   {}", memory.title);
    println!("Scope:   {}", memory.scope);
    println!("Type:    {}", memory.memory_type.as_str());
    if let Some(ref author) = memory.author {
        println!("Author:  {}", author);
    }
    if !memory.tags.is_empty() {
        println!("Tags:    {}", memory.tags.join(", "));
    }
    println!("Updated: {}", memory.updated_at);
    println!();
    let content = if memory.content.len() > 500 {
        format!("{}...", &memory.content[..500])
    } else {
        memory.content.clone()
    };
    println!("{}", content);
}

pub fn print_memories(results: &[SearchResult]) {
    if results.is_empty() {
        println!("No memories found.");
        return;
    }
    println!("{:<36}  {:<30}  {:<20}  Tags", "ID", "Title", "Scope");
    println!("{}", "-".repeat(100));
    for r in results {
        let title = if r.title.len() > 28 {
            format!("{}...", &r.title[..28])
        } else {
            r.title.clone()
        };
        let scope = if r.scope.len() > 18 {
            format!("{}...", &r.scope[..18])
        } else {
            r.scope.clone()
        };
        let tags = r.tags.join(", ");
        println!("{:<36}  {:<30}  {:<20}  {}", r.id, title, scope, tags);
    }
}

pub fn print_graph(graph: &Graph) {
    if graph.entities.is_empty() && graph.relations.is_empty() {
        println!("Empty graph.");
        return;
    }
    println!("Entities ({}):", graph.entities.len());
    for entity in &graph.entities {
        println!(
            "  [{}] {} ({})",
            entity.entity_type, entity.name, entity.scope
        );
        for obs in &entity.observations {
            println!("    - {}", obs.content);
        }
    }
    if !graph.relations.is_empty() {
        println!();
        println!("Relations ({}):", graph.relations.len());
        for rel in &graph.relations {
            println!(
                "  {} --[{}]--> {}",
                rel.from_entity, rel.relation_type, rel.to_entity
            );
        }
    }
}
