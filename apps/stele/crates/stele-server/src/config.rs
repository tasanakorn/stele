use clap::Parser;

#[cfg(feature = "stylos")]
use crate::settings::StyloSettings;

#[derive(Parser, Debug, Clone)]
#[command(name = "stele", version = concat!(env!("CARGO_PKG_VERSION"), " (", env!("GIT_HASH"), ")"), about = "Shared memory layer for Claude Code via MCP")]
pub struct Config {
    /// Address to bind the server to
    #[arg(long, default_value = "127.0.0.1:3100", env = "STELE_BIND")]
    pub bind: String,

    /// Path to the SQLite database file
    #[arg(long, default_value = "./stele.db", env = "STELE_DB")]
    pub db: String,

    /// MCP endpoint path
    #[arg(long, default_value = "/mcp", env = "STELE_MCP_PATH")]
    pub mcp_path: String,

    /// Open settings dialog (desktop only, used internally)
    #[arg(long, hide = true)]
    pub settings: bool,

    /// Pre-shared auth key. Clients must send it as `X-Stele-Key`.
    /// Overrides both STELE_AUTH_KEY and config.toml.
    #[arg(long, env = "STELE_AUTH_KEY")]
    pub auth_key: Option<String>,

    // ── Stylos overrides ──
    /// Enable the stylos/zenoh session. Overrides config.toml.
    #[arg(long = "stylos", env = "STELE_STYLOS_ENABLED")]
    pub stylos_enabled: Option<bool>,

    /// Disable the stylos/zenoh session (short form). Wins over --stylos / env.
    #[arg(long = "no-stylos", conflicts_with = "stylos_enabled")]
    pub no_stylos: bool,

    /// Stylos zenoh mode: peer | router | client.
    #[arg(long = "stylos-mode", env = "STELE_STYLOS_MODE")]
    pub stylos_mode: Option<String>,

    /// Stylos realm (first addressing segment).
    #[arg(long = "stylos-realm", env = "STELE_STYLOS_REALM")]
    pub stylos_realm: Option<String>,

    /// Stylos instance override. Default: derived from hostname.
    #[arg(long = "stylos-instance", env = "STELE_STYLOS_INSTANCE")]
    pub stylos_instance: Option<String>,

    /// Stylos connect endpoints (comma-separated, e.g. "tcp/10.0.0.5:31747").
    #[arg(
        long = "stylos-connect",
        env = "STELE_STYLOS_CONNECT",
        value_delimiter = ','
    )]
    pub stylos_connect: Vec<String>,
}

#[cfg(feature = "desktop")]
impl Config {
    /// Apply desktop-friendly defaults: move DB to ~/Library/Application Support/Stele/
    /// if the user hasn't overridden it via CLI or env var.
    pub fn with_desktop_defaults(mut self) -> Self {
        if self.db == "./stele.db" {
            if let Some(data_dir) = dirs::data_dir() {
                let stele_dir = data_dir.join("Stele");
                if std::fs::create_dir_all(&stele_dir).is_ok() {
                    self.db = stele_dir.join("stele.db").to_string_lossy().into_owned();
                }
            }
        }
        self
    }
}

#[cfg(feature = "stylos")]
impl Config {
    /// Merge CLI/env overrides into the file-loaded StyloSettings base.
    /// Precedence: `--no-stylos` > CLI/env > file > default.
    pub fn merge_stylos(&self, mut base: StyloSettings) -> StyloSettings {
        if self.no_stylos {
            base.enabled = false;
        } else if let Some(v) = self.stylos_enabled {
            base.enabled = v;
        }
        if let Some(m) = &self.stylos_mode {
            base.mode = m.clone();
        }
        if let Some(r) = &self.stylos_realm {
            base.realm = r.clone();
        }
        if let Some(i) = &self.stylos_instance {
            base.instance = Some(i.clone());
        }
        if !self.stylos_connect.is_empty() {
            base.connect = self.stylos_connect.clone();
        }
        base
    }
}
