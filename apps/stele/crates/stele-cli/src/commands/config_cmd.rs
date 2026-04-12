use crate::config::{config_path, load_config, save_config, Profile, SteleConfig};

pub fn handle_config_init() {
    let path = config_path();
    if path.exists() {
        println!("Config already exists at: {}", path.display());
        return;
    }
    let config = SteleConfig::default();
    match save_config(&config) {
        Ok(()) => println!("Config created at: {}", path.display()),
        Err(e) => {
            eprintln!("Error creating config: {}", e);
            std::process::exit(1);
        }
    }
}

pub fn handle_config_show() {
    let config = load_config();
    match toml::to_string_pretty(&config) {
        Ok(s) => println!("{}", s),
        Err(e) => {
            eprintln!("Error serializing config: {}", e);
            std::process::exit(1);
        }
    }
}

pub fn handle_config_path() {
    println!("{}", config_path().display());
}

pub fn handle_config_set(name: &str, url: &str, key: Option<String>, set_default: bool) {
    let mut config = load_config();
    // Preserve the existing host (set by the backfill on first load) when
    // updating a profile — `None` here would silently erase it.
    let existing_host = config
        .profiles
        .get(name)
        .and_then(|p| p.host.clone());
    config.profiles.insert(
        name.to_string(),
        Profile {
            server_url: url.to_string(),
            auth_key: key,
            host: existing_host,
        },
    );
    if set_default {
        config.default_profile = name.to_string();
    }
    match save_config(&config) {
        Ok(()) => println!("Profile '{}' saved ({})", name, url),
        Err(e) => {
            eprintln!("Error saving config: {}", e);
            std::process::exit(1);
        }
    }
}

pub fn handle_config_remove(name: &str) {
    let mut config = load_config();
    if config.profiles.remove(name).is_none() {
        eprintln!("Profile '{}' not found", name);
        std::process::exit(1);
    }
    if config.default_profile == name {
        config.default_profile = config
            .profiles
            .keys()
            .next()
            .cloned()
            .unwrap_or_else(|| "local".to_string());
        eprintln!(
            "Default profile was '{}', changed to '{}'",
            name, config.default_profile
        );
    }
    match save_config(&config) {
        Ok(()) => println!("Profile '{}' removed", name),
        Err(e) => {
            eprintln!("Error saving config: {}", e);
            std::process::exit(1);
        }
    }
}
