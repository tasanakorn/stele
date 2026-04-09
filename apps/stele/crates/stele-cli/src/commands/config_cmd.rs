use crate::config::{config_path, load_config, save_config, SteleConfig};

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
