use crate::client::SteleClient;
use crate::output::print_json;

pub fn handle_scopes(client: &SteleClient, prefix: Option<&str>, json: bool) {
    match client.list_scopes(prefix) {
        Ok(scopes) => {
            if json {
                let v = serde_json::to_value(&scopes).unwrap_or_default();
                print_json(&v);
            } else if scopes.is_empty() {
                println!("No scopes found.");
            } else {
                println!("{:<40}  Memories", "Scope");
                println!("{}", "-".repeat(52));
                for s in &scopes {
                    println!("{:<40}  {}", s.scope, s.count);
                }
            }
        }
        Err(e) => {
            eprintln!("Error: {}", e);
            std::process::exit(1);
        }
    }
}

pub fn handle_tags(client: &SteleClient, scope: Option<&str>, json: bool) {
    match client.list_tags(scope) {
        Ok(tags) => {
            if json {
                let v = serde_json::to_value(&tags).unwrap_or_default();
                print_json(&v);
            } else if tags.is_empty() {
                println!("No tags found.");
            } else {
                println!("{:<30}  Memories", "Tag");
                println!("{}", "-".repeat(42));
                for t in &tags {
                    println!("{:<30}  {}", t.tag, t.count);
                }
            }
        }
        Err(e) => {
            eprintln!("Error: {}", e);
            std::process::exit(1);
        }
    }
}

pub fn handle_stats(client: &SteleClient, json: bool) {
    match client.get_stats() {
        Ok(v) => {
            if json {
                print_json(&v);
            } else {
                if let Some(obj) = v.as_object() {
                    for (k, val) in obj {
                        println!("{}: {}", k, val);
                    }
                } else {
                    print_json(&v);
                }
            }
        }
        Err(e) => {
            eprintln!("Error: {}", e);
            std::process::exit(1);
        }
    }
}

pub fn handle_status(client: &SteleClient, json: bool) {
    let ok = client.health_check();
    if json {
        let v = serde_json::json!({ "status": if ok { "ok" } else { "unreachable" } });
        crate::output::print_json(&v);
    } else if ok {
        println!("Server is reachable.");
    } else {
        eprintln!("Server is unreachable.");
        std::process::exit(1);
    }
}
