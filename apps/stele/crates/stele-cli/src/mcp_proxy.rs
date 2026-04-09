use std::io::{self, BufRead, BufReader, Read, Write};
use ureq::Agent;

/// Run the MCP stdio-to-Streamable-HTTP proxy.
///
/// Reads JSON-RPC messages from stdin (one per line), POSTs each to the
/// server's `/mcp` endpoint, parses SSE responses, and writes JSON-RPC
/// messages back to stdout. Tracks `mcp-session-id` for session continuity.
pub fn run(server_url: String, auth_key: Option<String>) {
    let agent = Agent::new();
    let mcp_url = format!("{}/mcp", server_url.trim_end_matches('/'));
    let mut session_id: Option<String> = None;

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

        let mut req = agent
            .post(&mcp_url)
            .set("Content-Type", "application/json")
            .set("Accept", "application/json, text/event-stream");

        if let Some(ref sid) = session_id {
            req = req.set("mcp-session-id", sid);
        }
        if let Some(ref key) = auth_key {
            req = req.set("X-Stele-Key", key);
        }

        let resp = match req.send_string(trimmed) {
            Ok(r) => r,
            Err(ureq::Error::Status(code, resp)) => {
                let body = resp.into_string().unwrap_or_default();
                write_jsonrpc_error(&mut writer, trimmed, code, &body);
                continue;
            }
            Err(ureq::Error::Transport(e)) => {
                write_jsonrpc_error(&mut writer, trimmed, 502, &e.to_string());
                continue;
            }
        };

        if let Some(sid) = resp.header("mcp-session-id") {
            session_id = Some(sid.to_string());
        }

        let content_type = resp.header("content-type").unwrap_or("").to_string();

        if content_type.contains("text/event-stream") {
            parse_sse_to_stdout(resp.into_reader(), &mut writer);
        } else {
            let body = resp.into_string().unwrap_or_default();
            if !body.trim().is_empty() {
                let _ = writeln!(writer, "{}", body.trim());
                let _ = writer.flush();
            }
        }
    }

    // Clean shutdown: terminate MCP session
    if let Some(ref sid) = session_id {
        let mut req = agent.delete(&mcp_url).set("mcp-session-id", sid);
        if let Some(ref key) = auth_key {
            req = req.set("X-Stele-Key", key);
        }
        let _ = req.call();
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
