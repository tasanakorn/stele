//! Stylos CLI: pub / sub / get / queryable / identity.

use clap::{Args, Parser, Subcommand};
use std::path::PathBuf;
use std::time::Duration;
use stylos_config::StylosConfig;
use stylos_session::{log_session_info, open_session, SessionOverrides};
use zenoh::bytes::Encoding;

#[derive(Parser, Debug)]
#[command(name = "stylos", version, about = "Stylos — zenoh pub/sub/get/queryable CLI")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Args, Debug, Clone)]
struct CommonArgs {
    #[arg(long, global = true)]
    config: Option<PathBuf>,
    #[arg(long, global = true, num_args = 0..)]
    connect: Vec<String>,
    #[arg(long, global = true)]
    no_quic: bool,
}

#[derive(Subcommand, Debug)]
enum Command {
    Pub {
        key: String,
        msg: String,
        #[arg(long)] encoding: Option<String>,
        #[command(flatten)] common: CommonArgs,
    },
    Sub {
        key: String,
        #[command(flatten)] common: CommonArgs,
    },
    Get {
        key: String,
        #[arg(long, default_value_t = 3000)] timeout_ms: u64,
        #[command(flatten)] common: CommonArgs,
    },
    Queryable {
        key: String,
        #[arg(long, default_value = "reply-from-rust")] payload: String,
        #[arg(long)] complete: bool,
        #[command(flatten)] common: CommonArgs,
    },
    Identity {
        #[command(flatten)] common: CommonArgs,
    },
}

impl Command {
    fn common(&self) -> &CommonArgs {
        match self {
            Command::Pub { common, .. }
            | Command::Sub { common, .. }
            | Command::Get { common, .. }
            | Command::Queryable { common, .. }
            | Command::Identity { common, .. } => common,
        }
    }
}

fn load_config(common: &CommonArgs) -> anyhow::Result<StylosConfig> {
    Ok(match &common.config {
        Some(p) => StylosConfig::load(p)?,
        None => StylosConfig::load_default()?,
    })
}

fn overrides_from(common: &CommonArgs) -> SessionOverrides {
    SessionOverrides {
        connect: if common.connect.is_empty() { None } else { Some(common.connect.clone()) },
        no_quic: common.no_quic,
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    zenoh::init_log_from_env_or("error");
    let cli = Cli::parse();
    let common = cli.command.common().clone();
    let cfg = load_config(&common)?;
    let ov = overrides_from(&common);

    match cli.command {
        Command::Identity { .. } => {
            let id = cfg.stylos.to_identity()?;
            println!("realm    = {}", id.realm.as_str());
            println!("role     = {}", id.role.as_str());
            println!("instance = {}", id.instance.as_str());
            println!("root_key = {}", id.root_key());
            Ok(())
        }

        Command::Pub { key, msg, encoding, .. } => {
            let session = open_session(&cfg, &ov).await?;
            log_session_info(&session).await;
            let enc = match encoding.as_deref() {
                Some("text/plain") | None => Encoding::TEXT_PLAIN,
                Some(other) => Encoding::from(other.to_string()),
            };
            session.put(&key, msg.clone().into_bytes()).encoding(enc).await
                .map_err(|e| anyhow::anyhow!("put failed: {e}"))?;
            eprintln!("[pub] {} -> {}", key, msg);
            session.close().await.map_err(|e| anyhow::anyhow!("close: {e}"))?;
            Ok(())
        }

        Command::Sub { key, .. } => {
            let session = open_session(&cfg, &ov).await?;
            log_session_info(&session).await;
            let sub = session.declare_subscriber(&key).await
                .map_err(|e| anyhow::anyhow!("declare_subscriber: {e}"))?;
            eprintln!("[sub] listening on {key}; Ctrl-C to exit");
            loop {
                tokio::select! {
                    _ = tokio::signal::ctrl_c() => break,
                    res = sub.recv_async() => match res {
                        Ok(sample) => {
                            let ke = sample.key_expr().as_str().to_string();
                            let payload = sample.payload().try_to_string()
                                .unwrap_or_else(|e| format!("<{e}>").into());
                            println!("{ke}\t{payload}");
                        }
                        Err(e) => { eprintln!("[sub] recv err: {e}"); break; }
                    }
                }
            }
            session.close().await.map_err(|e| anyhow::anyhow!("close: {e}"))?;
            Ok(())
        }

        Command::Get { key, timeout_ms, .. } => {
            let session = open_session(&cfg, &ov).await?;
            log_session_info(&session).await;
            let replies = session.get(&key).await
                .map_err(|e| anyhow::anyhow!("get: {e}"))?;
            let deadline = tokio::time::Instant::now() + Duration::from_millis(timeout_ms);
            let mut count = 0usize;
            loop {
                let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
                if remaining.is_zero() { break; }
                match tokio::time::timeout(remaining, replies.recv_async()).await {
                    Err(_) => break,
                    Ok(Err(_)) => break,
                    Ok(Ok(reply)) => {
                        count += 1;
                        match reply.result() {
                            Ok(sample) => {
                                let ke = sample.key_expr().as_str().to_string();
                                let payload = sample.payload().try_to_string()
                                    .unwrap_or_else(|e| format!("<{e}>").into());
                                println!("{ke}\t{payload}");
                            }
                            Err(e) => eprintln!("[get] reply err: {:?}", e),
                        }
                    }
                }
            }
            eprintln!("[get] {count} replies");
            session.close().await.map_err(|e| anyhow::anyhow!("close: {e}"))?;
            Ok(())
        }

        Command::Queryable { key, payload, complete, .. } => {
            let session = open_session(&cfg, &ov).await?;
            log_session_info(&session).await;
            let q = session.declare_queryable(&key).complete(complete).await
                .map_err(|e| anyhow::anyhow!("declare_queryable: {e}"))?;
            eprintln!("[queryable] serving {key} (complete={complete}); Ctrl-C to exit");
            loop {
                tokio::select! {
                    _ = tokio::signal::ctrl_c() => break,
                    res = q.recv_async() => match res {
                        Ok(query) => {
                            eprintln!("[queryable] got query: {}", query.selector());
                            if let Err(e) = query.reply(key.clone(), payload.clone().into_bytes()).await {
                                eprintln!("[queryable] reply err: {e}");
                            }
                        }
                        Err(e) => { eprintln!("[queryable] recv err: {e}"); break; }
                    }
                }
            }
            session.close().await.map_err(|e| anyhow::anyhow!("close: {e}"))?;
            Ok(())
        }
    }
}
