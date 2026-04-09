use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SteleConfig {
    pub default_profile: String,
    pub profiles: HashMap<String, Profile>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Profile {
    pub server_url: String,
    pub auth_key: Option<String>,
}

impl Default for SteleConfig {
    fn default() -> Self {
        let mut profiles = HashMap::new();
        profiles.insert(
            "local".to_string(),
            Profile {
                server_url: "http://127.0.0.1:3100".to_string(),
                auth_key: None,
            },
        );
        Self {
            default_profile: "local".to_string(),
            profiles,
        }
    }
}

pub fn config_path() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("stele")
        .join("config.toml")
}

pub fn load_config() -> SteleConfig {
    let path = config_path();
    if !path.exists() {
        return SteleConfig::default();
    }
    let contents = match std::fs::read_to_string(&path) {
        Ok(s) => s,
        Err(_) => return SteleConfig::default(),
    };
    toml::from_str(&contents).unwrap_or_default()
}

pub fn save_config(config: &SteleConfig) -> Result<(), String> {
    let path = config_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let contents = toml::to_string_pretty(config).map_err(|e| e.to_string())?;
    std::fs::write(&path, contents).map_err(|e| e.to_string())?;
    Ok(())
}

pub struct CliArgs {
    pub profile: Option<String>,
    pub server_url: Option<String>,
    pub auth_key: Option<String>,
}

pub fn resolve_connection(cli_args: &CliArgs) -> (String, Option<String>) {
    // CLI flags > env vars > profile from config > defaults
    let url = if let Some(ref u) = cli_args.server_url {
        u.clone()
    } else if let Ok(u) = std::env::var("STELE_URL") {
        u
    } else {
        let config = load_config();
        let profile_name = cli_args
            .profile
            .clone()
            .or_else(|| std::env::var("STELE_PROFILE").ok())
            .unwrap_or_else(|| config.default_profile.clone());
        config
            .profiles
            .get(&profile_name)
            .map(|p| p.server_url.clone())
            .unwrap_or_else(|| "http://localhost:3100".to_string())
    };

    let key = if let Some(ref k) = cli_args.auth_key {
        Some(k.clone())
    } else if let Ok(k) = std::env::var("STELE_AUTH_KEY") {
        Some(k)
    } else {
        let config = load_config();
        let profile_name = cli_args
            .profile
            .clone()
            .or_else(|| std::env::var("STELE_PROFILE").ok())
            .unwrap_or_else(|| config.default_profile.clone());
        config
            .profiles
            .get(&profile_name)
            .and_then(|p| p.auth_key.clone())
    };

    (url, key)
}
