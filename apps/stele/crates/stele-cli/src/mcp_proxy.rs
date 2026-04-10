use std::collections::HashMap;
use std::io::{self, BufRead, BufReader, Read, Write};
use ureq::Agent;

use crate::config::SteleConfig;

/// Run the MCP stdio-to-Streamable-HTTP proxy with local tool handling.
///
/// Reads JSON-RPC messages from stdin, dispatches local tools (e.g. `list_profiles`)
/// directly, and forwards all other requests to the server's `/mcp` endpoint.
/// Supports per-call `profile` parameter to route to different servers.
/// Tracks `mcp-session-id` per server URL for session continuity.
pub fn run(config: SteleConfig, default_profile: String) {
    let agent = Agent::new();

    // Resolve the default server connection
    let (default_url, default_key) = resolve_profile(&config, &default_profile);
    let default_mcp_url = format!("{}/mcp", default_url.trim_end_matches('/'));

    // Per-server session tracking: mcp_url -> (session_id, auth_key)
    let mut sessions: HashMap<String, (String, Option<String>)> = HashMap::new();

    let stdin = io::stdin();
    let reader = BufReader::new(stdin.lock());
    let stdout = io::stdout();
    let mut writer = stdout.lock();

    for line in reader.lines() {
        let line = match line {
            Ok(l) => l,
            Err(_) => break,
        };

        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        // Parse JSON-RPC message
        let mut msg: serde_json::Value = match serde_json::from_str(trimmed) {
            Ok(v) => v,
            Err(_) => {
                write_jsonrpc_error(&mut writer, trimmed, 400, "Invalid JSON");
                continue;
            }
        };

        let method = msg
            .get("method")
            .and_then(|m| m.as_str())
            .unwrap_or("")
            .to_string();

        match method.as_str() {
            "tools/call" => {
                let tool_name = msg
                    .pointer("/params/name")
                    .and_then(|n| n.as_str())
                    .unwrap_or("")
                    .to_string();

                if tool_name == "list_profiles" {
                    // Handle locally
                    let id = msg.get("id").cloned().unwrap_or(serde_json::Value::Null);
                    let resp = handle_list_profiles(&config, &default_profile, &id);
                    let _ = writeln!(writer, "{}", resp);
                    let _ = writer.flush();
                    continue;
                }

                // Extract optional profile param, resolve target server
                let profile_override = extract_profile_param(&mut msg);
                let (mcp_url, auth_key) = if let Some(ref pname) = profile_override {
                    if !config.profiles.contains_key(pname.as_str()) {
                        let id = msg.get("id").cloned().unwrap_or(serde_json::Value::Null);
                        let err = serde_json::json!({
                            "jsonrpc": "2.0",
                            "id": id,
                            "error": {
                                "code": -32602,
                                "message": format!("Unknown profile: {pname}")
                            }
                        });
                        let _ = writeln!(writer, "{}", err);
                        let _ = writer.flush();
                        continue;
                    }
                    let (url, key) = resolve_profile(&config, pname);
                    (
                        format!("{}/mcp", url.trim_end_matches('/')),
                        key,
                    )
                } else {
                    (default_mcp_url.clone(), default_key.clone())
                };

                forward_request(
                    &agent,
                    &mcp_url,
                    &auth_key,
                    &mut sessions,
                    &msg,
                    &mut writer,
                );
            }

            "tools/list" => {
                // Forward to server, then inject local tools into response
                let lines = forward_request_capture(
                    &agent,
                    &default_mcp_url,
                    &default_key,
                    &mut sessions,
                    &msg,
                );
                for line in lines {
                    if let Ok(mut parsed) = serde_json::from_str::<serde_json::Value>(&line) {
                        inject_local_tools(&mut parsed);
                        let _ = writeln!(writer, "{}", parsed);
                    } else {
                        let _ = writeln!(writer, "{}", line);
                    }
                }
                let _ = writer.flush();
            }

            _ => {
                // Forward everything else (initialize, notifications, etc.)
                forward_request(
                    &agent,
                    &default_mcp_url,
                    &default_key,
                    &mut sessions,
                    &msg,
                    &mut writer,
                );
            }
        }
    }

    // Clean shutdown: terminate all MCP sessions
    for (url, (sid, key)) in &sessions {
        let mut req = agent.delete(url).set("mcp-session-id", sid);
        if let Some(ref k) = key {
            req = req.set("X-Stele-Key", k);
        }
        let _ = req.call();
    }
}

/// Resolve a profile name to (server_url, auth_key).
fn resolve_profile(config: &SteleConfig, name: &str) -> (String, Option<String>) {
    config
        .profiles
        .get(name)
        .map(|p| (p.server_url.clone(), p.auth_key.clone()))
        .unwrap_or_else(|| ("http://localhost:3100".to_string(), None))
}

/// Extract and remove the `profile` parameter from tool call arguments.
fn extract_profile_param(msg: &mut serde_json::Value) -> Option<String> {
    let args = msg.pointer_mut("/params/arguments")?;
    let obj = args.as_object_mut()?;
    let val = obj.remove("profile")?;
    val.as_str().map(|s| s.to_string())
}

/// Build a JSON-RPC response for `list_profiles` (local tool).
fn handle_list_profiles(
    config: &SteleConfig,
    default_profile: &str,
    id: &serde_json::Value,
) -> serde_json::Value {
    let profiles: Vec<serde_json::Value> = config
        .profiles
        .iter()
        .map(|(name, profile)| {
            serde_json::json!({
                "name": name,
                "server_url": profile.server_url,
                "is_default": name == default_profile,
            })
        })
        .collect();

    serde_json::json!({
        "jsonrpc": "2.0",
        "id": id,
        "result": {
            "content": [{
                "type": "text",
                "text": serde_json::to_string_pretty(&profiles).unwrap_or_else(|_| "[]".to_string())
            }]
        }
    })
}

/// Inject local tool definitions into a `tools/list` response.
fn inject_local_tools(response: &mut serde_json::Value) {
    let tools = match response.pointer_mut("/result/tools") {
        Some(t) if t.is_array() => t.as_array_mut().unwrap(),
        _ => return,
    };

    // Add `profile` property to every server tool's input schema
    for tool in tools.iter_mut() {
        if let Some(schema) = tool.pointer_mut("/inputSchema/properties") {
            if let Some(obj) = schema.as_object_mut() {
                obj.insert(
                    "profile".to_string(),
                    serde_json::json!({
                        "type": "string",
                        "description": "Connection profile name to route this call to (optional, uses default profile if omitted)"
                    }),
                );
            }
        }
    }

    // Append `list_profiles` tool
    tools.push(serde_json::json!({
        "name": "list_profiles",
        "description": "List configured Stele connection profiles. Returns profile names, server URLs, and which is the default. This tool is handled locally by the CLI and never forwarded to the server.",
        "inputSchema": {
            "type": "object",
            "properties": {},
            "required": []
        }
    }));
}

/// Forward a JSON-RPC request to the server and write the response to stdout.
fn forward_request(
    agent: &Agent,
    mcp_url: &str,
    auth_key: &Option<String>,
    sessions: &mut HashMap<String, (String, Option<String>)>,
    msg: &serde_json::Value,
    writer: &mut impl Write,
) {
    let body = serde_json::to_string(msg).unwrap_or_default();

    let mut req = agent
        .post(mcp_url)
        .set("Content-Type", "application/json")
        .set("Accept", "application/json, text/event-stream");

    if let Some((sid, _)) = sessions.get(mcp_url) {
        req = req.set("mcp-session-id", sid);
    }
    if let Some(ref key) = auth_key {
        req = req.set("X-Stele-Key", key);
    }

    let resp = match req.send_string(&body) {
        Ok(r) => r,
        Err(ureq::Error::Status(code, resp)) => {
            let err_body = resp.into_string().unwrap_or_default();
            write_jsonrpc_error(writer, &body, code, &err_body);
            return;
        }
        Err(ureq::Error::Transport(e)) => {
            write_jsonrpc_error(writer, &body, 502, &e.to_string());
            return;
        }
    };

    if let Some(sid) = resp.header("mcp-session-id") {
        sessions.insert(mcp_url.to_string(), (sid.to_string(), auth_key.clone()));
    }

    let content_type = resp.header("content-type").unwrap_or("").to_string();

    if content_type.contains("text/event-stream") {
        parse_sse_to_stdout(resp.into_reader(), writer);
    } else {
        let body = resp.into_string().unwrap_or_default();
        if !body.trim().is_empty() {
            let _ = writeln!(writer, "{}", body.trim());
            let _ = writer.flush();
        }
    }
}

/// Forward a JSON-RPC request and capture response lines instead of writing to stdout.
fn forward_request_capture(
    agent: &Agent,
    mcp_url: &str,
    auth_key: &Option<String>,
    sessions: &mut HashMap<String, (String, Option<String>)>,
    msg: &serde_json::Value,
) -> Vec<String> {
    let body = serde_json::to_string(msg).unwrap_or_default();

    let mut req = agent
        .post(mcp_url)
        .set("Content-Type", "application/json")
        .set("Accept", "application/json, text/event-stream");

    if let Some((sid, _)) = sessions.get(mcp_url) {
        req = req.set("mcp-session-id", sid);
    }
    if let Some(ref key) = auth_key {
        req = req.set("X-Stele-Key", key);
    }

    let resp = match req.send_string(&body) {
        Ok(r) => r,
        Err(ureq::Error::Status(code, resp)) => {
            let err_body = resp.into_string().unwrap_or_default();
            let id = serde_json::from_str::<serde_json::Value>(&body)
                .ok()
                .and_then(|v| v.get("id").cloned())
                .unwrap_or(serde_json::Value::Null);
            let err = serde_json::json!({
                "jsonrpc": "2.0",
                "id": id,
                "error": {
                    "code": -32000,
                    "message": format!("HTTP {code}: {err_body}")
                }
            });
            return vec![err.to_string()];
        }
        Err(ureq::Error::Transport(e)) => {
            let id = serde_json::from_str::<serde_json::Value>(&body)
                .ok()
                .and_then(|v| v.get("id").cloned())
                .unwrap_or(serde_json::Value::Null);
            let err = serde_json::json!({
                "jsonrpc": "2.0",
                "id": id,
                "error": {
                    "code": -32000,
                    "message": format!("HTTP 502: {e}")
                }
            });
            return vec![err.to_string()];
        }
    };

    if let Some(sid) = resp.header("mcp-session-id") {
        sessions.insert(mcp_url.to_string(), (sid.to_string(), auth_key.clone()));
    }

    let content_type = resp.header("content-type").unwrap_or("").to_string();

    if content_type.contains("text/event-stream") {
        parse_sse_to_lines(resp.into_reader())
    } else {
        let body = resp.into_string().unwrap_or_default();
        let trimmed = body.trim();
        if trimmed.is_empty() {
            vec![]
        } else {
            vec![trimmed.to_string()]
        }
    }
}

fn parse_sse_to_stdout(reader: impl Read, writer: &mut impl Write) {
    let buf = BufReader::new(reader);
    let mut data_buf = String::new();

    for line in buf.lines() {
        let line = match line {
            Ok(l) => l,
            Err(_) => break,
        };

        if let Some(data) = line.strip_prefix("data: ") {
            if !data_buf.is_empty() {
                data_buf.push('\n');
            }
            data_buf.push_str(data);
        } else if line.is_empty() && !data_buf.is_empty() {
            let _ = writeln!(writer, "{}", data_buf);
            let _ = writer.flush();
            data_buf.clear();
        }
        // Ignore event:, id:, retry:, and comment lines
    }

    // Flush remaining data if server closed without trailing blank line
    if !data_buf.is_empty() {
        let _ = writeln!(writer, "{}", data_buf);
        let _ = writer.flush();
    }
}

/// Parse SSE stream into collected lines (for response interception).
fn parse_sse_to_lines(reader: impl Read) -> Vec<String> {
    let buf = BufReader::new(reader);
    let mut data_buf = String::new();
    let mut lines = Vec::new();

    for line in buf.lines() {
        let line = match line {
            Ok(l) => l,
            Err(_) => break,
        };

        if let Some(data) = line.strip_prefix("data: ") {
            if !data_buf.is_empty() {
                data_buf.push('\n');
            }
            data_buf.push_str(data);
        } else if line.is_empty() && !data_buf.is_empty() {
            lines.push(data_buf.clone());
            data_buf.clear();
        }
    }

    if !data_buf.is_empty() {
        lines.push(data_buf);
    }

    lines
}

fn write_jsonrpc_error(writer: &mut impl Write, request_line: &str, status: u16, message: &str) {
    let id = serde_json::from_str::<serde_json::Value>(request_line)
        .ok()
        .and_then(|v| v.get("id").cloned())
        .unwrap_or(serde_json::Value::Null);

    let error_resp = serde_json::json!({
        "jsonrpc": "2.0",
        "id": id,
        "error": {
            "code": -32000,
            "message": format!("HTTP {status}: {message}")
        }
    });

    let _ = writeln!(writer, "{error_resp}");
    let _ = writer.flush();
}
