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
}
