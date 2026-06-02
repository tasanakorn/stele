//! `stele mail` surface (PRD-027 §4.15). Talks to the local node over
//! stylos/zenoh queryables, prints the raw JSON reply, maps `ok`/`error` to
//! exit codes.

use clap::Subcommand;

use crate::stylos_client::StylosClient;

#[derive(Subcommand)]
pub enum MailCommands {
    #[command(about = "Send a message to a home (host:project)")]
    Send {
        #[arg(long, required = true)]
        to_host: String,
        #[arg(long, required = true)]
        to_project: String,
        #[arg(long)]
        attention: Option<String>,
        #[arg(long)]
        subject: Option<String>,
        #[arg(long = "type")]
        message_type: Option<String>,
        #[arg(long)]
        meta: Option<String>,
        #[arg(long)]
        payload: Option<String>,
        #[arg(long)]
        from_alias: Option<String>,
    },

    #[command(about = "List inbox messages visible to the caller")]
    List {
        #[arg(long)]
        alias: Vec<String>,
        #[arg(long, value_delimiter = ',')]
        status: Option<Vec<String>>,
        #[arg(long)]
        project: Option<String>,
    },

    #[command(about = "Mark a message read (NEW -> READ)")]
    Read { message_id: i64 },

    #[command(about = "Archive a message (NEW|READ -> ARCHIVE)")]
    Archive { message_id: i64 },

    #[command(about = "Fetch one message by id")]
    Get { message_id: i64 },

    #[command(about = "Inspect this node's outbound spool")]
    Outbox {
        #[arg(long, value_delimiter = ',')]
        status: Option<Vec<String>>,
    },

    #[command(about = "Register attention aliases for a project")]
    Register {
        #[arg(long, required = true)]
        alias: Vec<String>,
        #[arg(long)]
        project: Option<String>,
    },
}

/// Connection info resolved by the sync path and handed to the mail runtime.
pub struct MailConn {
    pub host: Option<String>,
    pub zenoh_endpoint: String,
    pub realm: String,
}

/// Sanitize an id segment like steop's `sanitizeHeader`: map `:` (the composite
/// separator) to `-`, keep graphic ASCII, drop spaces/control/non-ASCII.
fn sanitize(s: &str) -> String {
    s.chars()
        .map(|c| if c == ':' { '-' } else { c })
        .filter(|c| c.is_ascii_graphic())
        .collect()
}

/// Detected project dir: `CLAUDE_PROJECT_DIR` only, sanitized. Matches steop —
/// no `PWD` fallback, which would silently target the wrong project's mailbox
/// when run outside a Claude Code session. When absent, the caller must pass
/// `--project` explicitly.
fn detect_project_dir() -> Option<String> {
    std::env::var("CLAUDE_PROJECT_DIR")
        .ok()
        .filter(|s| !s.is_empty())
        .map(|s| sanitize(&s))
}

/// Local host for the `from` segment: `STELE_HOST` env override (steop's
/// precedence), else the resolved connection host. Left un-sanitized so it
/// equals the node's `mailbox_host` claim and replies route back correctly.
fn local_host(conn: &MailConn) -> Option<String> {
    std::env::var("STELE_HOST")
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .or_else(|| conn.host.clone())
}

fn resolve_project(explicit: &Option<String>) -> Result<String, String> {
    if let Some(p) = explicit {
        return Ok(sanitize(p));
    }
    detect_project_dir().ok_or_else(|| "could not detect project dir (set --project)".to_string())
}

/// Exit codes per §4.15.
const EXIT_OK: i32 = 0;
const EXIT_TRANSPORT: i32 = 1;
const EXIT_BAD_REQUEST: i32 = 2;
const EXIT_NOT_FOUND: i32 = 3;
const EXIT_UNDELIVERABLE: i32 = 4;

fn exit_for_reply(reply: &serde_json::Value) -> i32 {
    if reply.get("ok").and_then(|v| v.as_bool()).unwrap_or(false) {
        return EXIT_OK;
    }
    match reply.get("error").and_then(|v| v.as_str()).unwrap_or("") {
        "not_found" => EXIT_NOT_FOUND,
        "undeliverable" | "conflict" => EXIT_UNDELIVERABLE,
        "bad_request" => EXIT_BAD_REQUEST,
        _ => EXIT_TRANSPORT,
    }
}

fn parse_json_arg(label: &str, raw: &Option<String>) -> Result<Option<serde_json::Value>, String> {
    match raw {
        None => Ok(None),
        Some(s) => serde_json::from_str(s)
            .map(Some)
            .map_err(|e| format!("invalid --{} JSON: {}", label, e)),
    }
}

/// Build the per-subcommand leaf name + request JSON. Performs client-side
/// validation (e.g. JSON parsing) that must fail before any network call.
fn build_request(
    cmd: &MailCommands,
    conn: &MailConn,
) -> Result<(&'static str, serde_json::Value), String> {
    match cmd {
        MailCommands::Send {
            to_host,
            to_project,
            attention,
            subject,
            message_type,
            meta,
            payload,
            from_alias,
        } => {
            let meta = parse_json_arg("meta", meta)?;
            let payload = parse_json_arg("payload", payload)?;

            let host = local_host(conn)
                .ok_or_else(|| "could not determine local host for `from`".to_string())?;
            let project = detect_project_dir().unwrap_or_default();
            let mut from = format!("{}:{}", host, project);
            if let Some(a) = from_alias {
                from = format!("{}:{}", from, a);
            }

            // Sanitize `to_project` the same way `list`/`register` sanitize
            // `--project`, so a send and a later list of the same home agree.
            // `to_host` is left raw to match the node's `mailbox_host` claim.
            let mut req = serde_json::json!({
                "to_host": to_host,
                "to_project": sanitize(&to_project),
                "from": from,
            });
            let obj = req.as_object_mut().unwrap();
            if let Some(a) = attention {
                obj.insert("attention".to_string(), serde_json::json!(a));
            }
            if let Some(s) = subject {
                obj.insert("subject".to_string(), serde_json::json!(s));
            }
            if let Some(t) = message_type {
                obj.insert("message_type".to_string(), serde_json::json!(t));
            }
            if let Some(m) = meta {
                obj.insert("meta".to_string(), m);
            }
            if let Some(p) = payload {
                obj.insert("payload".to_string(), p);
            }
            Ok(("send", req))
        }

        MailCommands::List {
            alias,
            status,
            project,
        } => {
            let project_dir = resolve_project(project)?;
            let statuses = status
                .clone()
                .unwrap_or_else(|| vec!["NEW".to_string(), "READ".to_string()]);
            let mut req = serde_json::json!({
                "project_dir": project_dir,
                "status": statuses,
            });
            if !alias.is_empty() {
                req.as_object_mut()
                    .unwrap()
                    .insert("aliases".to_string(), serde_json::json!(alias));
            }
            Ok(("list", req))
        }

        MailCommands::Register { alias, project } => {
            let project_dir = resolve_project(project)?;
            let req = serde_json::json!({
                "project_dir": project_dir,
                "aliases": alias,
                "register_only": true,
            });
            Ok(("list", req))
        }

        MailCommands::Read { message_id } => {
            Ok(("read", serde_json::json!({ "message_id": message_id })))
        }

        MailCommands::Archive { message_id } => {
            Ok(("archive", serde_json::json!({ "message_id": message_id })))
        }

        MailCommands::Get { message_id } => {
            Ok(("get", serde_json::json!({ "message_id": message_id })))
        }

        MailCommands::Outbox { status } => {
            let statuses = status
                .clone()
                .unwrap_or_else(|| vec!["QUEUED".to_string(), "DEAD".to_string()]);
            Ok(("outbox", serde_json::json!({ "status": statuses })))
        }
    }
}

/// Entry point for the `Mail` arm: build request, open peer, query, print, exit.
pub async fn run_mail(cmd: MailCommands, conn: MailConn) -> i32 {
    // Client-side validation (bad JSON etc.) must fail before any network call.
    let (leaf, request) = match build_request(&cmd, &conn) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("{}", e);
            return EXIT_BAD_REQUEST;
        }
    };

    let timing = std::env::var("STELE_MAIL_TIMING").is_ok();
    let t0 = std::time::Instant::now();
    let client = match StylosClient::open(&conn.zenoh_endpoint, &conn.realm).await {
        Ok(c) => c,
        Err(e) => {
            eprintln!("failed to open zenoh session: {}", e);
            return EXIT_TRANSPORT;
        }
    };
    if timing { eprintln!("[timing] open={:?}", t0.elapsed()); }

    let t1 = std::time::Instant::now();
    let instance = match client.resolve_local_instance().await {
        Ok(i) => i,
        Err(e) => {
            eprintln!("{}", e);
            client.close().await;
            return EXIT_TRANSPORT;
        }
    };
    if timing { eprintln!("[timing] resolve={:?}", t1.elapsed()); }

    let t2 = std::time::Instant::now();
    let reply = client.query_leaf(&instance, leaf, &request).await;
    if timing { eprintln!("[timing] query={:?}", t2.elapsed()); }
    let t3 = std::time::Instant::now();
    client.close().await;
    if timing { eprintln!("[timing] close={:?}", t3.elapsed()); }

    match reply {
        Ok(reply) => {
            println!("{}", serde_json::to_string(&reply).unwrap_or_default());
            exit_for_reply(&reply)
        }
        Err(e) => {
            eprintln!("transport error: {}", e);
            EXIT_TRANSPORT
        }
    }
}
