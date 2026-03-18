use clap::Parser;

#[derive(Parser, Debug, Clone)]
#[command(name = "stele", about = "Shared memory layer for Claude Code via MCP")]
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
