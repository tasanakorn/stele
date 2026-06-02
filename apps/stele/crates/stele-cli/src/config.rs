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
    #[serde(default)]
    pub auth_key: Option<String>,
    #[serde(default)]
    pub host: Option<String>,
    #[serde(default = "default_zenoh_endpoint")]
    pub zenoh_endpoint: String,
}

pub fn default_zenoh_endpoint() -> String {
    "tcp/127.0.0.1:31747".to_string()
}

impl Default for SteleConfig {
    fn default() -> Self {
        let mut profiles = HashMap::new();
        profiles.insert(
            "local".to_string(),
            Profile {
                server_url: "http://127.0.0.1:3100".to_string(),
                auth_key: None,
                host: None,
                zenoh_endpoint: default_zenoh_endpoint(),
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
    let existed = path.exists();
    if !existed {
        return SteleConfig::default();
    }
    let contents = match std::fs::read_to_string(&path) {
        Ok(s) => s,
        Err(_) => return SteleConfig::default(),
    };

    // Parse as a generic Table so unknown top-level keys (e.g. stele-server's
    // `[server]` section on macOS, where CLI and server share config.toml via
    // case-insensitive APFS) are ignored during deserialization but preserved
    // on save.
    let table: toml::Table = toml::from_str(&contents).unwrap_or_default();

    let default_profile = table
        .get("default_profile")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .unwrap_or_else(|| "local".to_string());

    let profiles: HashMap<String, Profile> = table
        .get("profiles")
        .cloned()
        .and_then(|v| v.try_into().ok())
        .unwrap_or_default();

    let mut config = if profiles.is_empty() {
        // File exists but has no CLI-side content yet (e.g. only `[server]`
        // from the daemon). Seed the default profile so the CLI remains usable.
        SteleConfig::default()
    } else {
        SteleConfig {
            default_profile,
            profiles,
        }
    };

    // Backfill missing per-profile host with the current machine's hostname.
    let raw = gethostname::gethostname().to_string_lossy().to_string();
    let cleaned: String = raw.chars().filter(|c| c.is_ascii_graphic()).collect();
    let hostname = if cleaned.is_empty() {
        "unknown".to_string()
    } else {
        cleaned
    };

    let mut mutated = false;
    for profile in config.profiles.values_mut() {
        if profile.host.is_none() {
            profile.host = Some(hostname.clone());
            mutated = true;
        }
    }

    if mutated && existed {
        let _ = save_config(&config);
    }

    config
}

pub fn save_config(config: &SteleConfig) -> Result<(), String> {
    let path = config_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }

    // Round-trip through a generic Table so unknown top-level keys written by
    // other binaries (notably stele-server's `[server]` section) are preserved.
    // The CLI owns only `default_profile` and `profiles` — any other keys stay
    // untouched.
    let mut merged: toml::Table = std::fs::read_to_string(&path)
        .ok()
        .and_then(|s| toml::from_str(&s).ok())
        .unwrap_or_default();

    merged.insert(
        "default_profile".to_string(),
        toml::Value::String(config.default_profile.clone()),
    );
    merged.insert(
        "profiles".to_string(),
        toml::Value::try_from(&config.profiles).map_err(|e| e.to_string())?,
    );

    let contents = toml::to_string_pretty(&merged).map_err(|e| e.to_string())?;
    std::fs::write(&path, contents).map_err(|e| e.to_string())?;
    Ok(())
}

pub struct CliArgs {
    pub profile: Option<String>,
    pub server_url: Option<String>,
    pub auth_key: Option<String>,
    pub zenoh_endpoint: Option<String>,
}

pub fn resolve_connection(
    cli_args: &CliArgs,
) -> (String, Option<String>, Option<String>, String) {
    // CLI flags > env vars > profile from config > defaults
    let config = load_config();
    let profile_name = cli_args
        .profile
        .clone()
        .or_else(|| std::env::var("STELE_PROFILE").ok())
        .unwrap_or_else(|| config.default_profile.clone());
    let profile = config.profiles.get(&profile_name);

    let url = if let Some(ref u) = cli_args.server_url {
        u.clone()
    } else if let Ok(u) = std::env::var("STELE_URL") {
        u
    } else {
        profile
            .map(|p| p.server_url.clone())
            .unwrap_or_else(|| "http://localhost:3100".to_string())
    };

    let key = if let Some(ref k) = cli_args.auth_key {
        Some(k.clone())
    } else if let Ok(k) = std::env::var("STELE_AUTH_KEY") {
        Some(k)
    } else {
        profile.and_then(|p| p.auth_key.clone())
    };

    let host = profile.and_then(|p| p.host.clone());

    let zenoh_endpoint = if let Some(ref e) = cli_args.zenoh_endpoint {
        e.clone()
    } else if let Ok(e) = std::env::var("STELE_ZENOH_ENDPOINT") {
        e
    } else {
        profile
            .map(|p| p.zenoh_endpoint.clone())
            .unwrap_or_else(default_zenoh_endpoint)
    };

    (url, key, host, zenoh_endpoint)
}
