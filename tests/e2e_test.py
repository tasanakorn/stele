#!/usr/bin/env python3
"""
End-to-end integration tests for Stele.

Builds the server and CLI, starts a headless server on a random port with
a temporary database, then exercises the REST API, CLI commands, and MCP
stdio proxy.

Usage:
    python tests/e2e_test.py              # build + test
    python tests/e2e_test.py --no-build   # skip build (use existing binaries)

Requires: Python 3.8+, no external dependencies.
"""

import json
import os
import signal
import socket
import subprocess
import sys
import tempfile
import time
import urllib.error
import urllib.request

# ---------------------------------------------------------------------------
# Config
# ---------------------------------------------------------------------------

WORKSPACE_DIR = os.path.join(os.path.dirname(__file__), "..", "apps", "stele")
WORKSPACE_DIR = os.path.abspath(WORKSPACE_DIR)
TARGET_DIR = os.path.join(WORKSPACE_DIR, "target", "debug")

SERVER_BIN = os.path.join(TARGET_DIR, "stele-server")
CLI_BIN = os.path.join(TARGET_DIR, "stele")

passed = 0
failed = 0
errors = []


# ---------------------------------------------------------------------------
# Helpers
# ---------------------------------------------------------------------------


def find_free_port():
    with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as s:
        s.bind(("127.0.0.1", 0))
        return s.getsockname()[1]


def wait_for_server(base_url, timeout=10):
    deadline = time.time() + timeout
    while time.time() < deadline:
        try:
            req = urllib.request.Request(f"{base_url}/api/v1/stats")
            with urllib.request.urlopen(req, timeout=2):
                return True
        except Exception:
            time.sleep(0.2)
    return False


def api(method, path, base_url, body=None):
    url = f"{base_url}{path}"
    data = json.dumps(body).encode() if body else None
    req = urllib.request.Request(url, data=data, method=method)
    req.add_header("Content-Type", "application/json")
    try:
        with urllib.request.urlopen(req, timeout=10) as resp:
            return resp.status, json.loads(resp.read())
    except urllib.error.HTTPError as e:
        return e.code, json.loads(e.read())


def cli(*args, env_extra=None, json_output=False):
    env = os.environ.copy()
    if env_extra:
        env.update(env_extra)
    # --json is a global flag that must come before the subcommand
    cmd = [CLI_BIN]
    if json_output:
        cmd.append("--json")
    cmd.extend(args)
    result = subprocess.run(
        cmd, capture_output=True, text=True, timeout=15, env=env,
    )
    return result.returncode, result.stdout, result.stderr


def mcp_call(messages, base_url, timeout=15):
    """Send MCP JSON-RPC messages via the stdio proxy and return responses."""
    input_text = "\n".join(json.dumps(m) for m in messages) + "\n"
    result = subprocess.run(
        [CLI_BIN, "--server-url", base_url, "mcp"],
        input=input_text, capture_output=True, text=True, timeout=timeout,
    )
    responses = []
    for line in result.stdout.strip().splitlines():
        line = line.strip()
        if line:
            try:
                responses.append(json.loads(line))
            except json.JSONDecodeError:
                pass
    return responses


def test(name, condition, detail=""):
    global passed, failed
    if condition:
        passed += 1
        print(f"  PASS  {name}")
    else:
        failed += 1
        msg = f"  FAIL  {name}"
        if detail:
            msg += f"  ({detail})"
        print(msg)
        errors.append(name)


# ---------------------------------------------------------------------------
# Build
# ---------------------------------------------------------------------------


def build():
    print("Building binaries...")
    result = subprocess.run(
        ["cargo", "build", "-p", "stele-server", "--features", "headless",
         "--no-default-features", "-p", "stele-cli"],
        cwd=WORKSPACE_DIR, capture_output=True, text=True, timeout=300,
    )
    if result.returncode != 0:
        print("Build failed:")
        print(result.stderr[-2000:] if len(result.stderr) > 2000 else result.stderr)
        sys.exit(1)
    print("Build OK\n")


# ---------------------------------------------------------------------------
# Test Suites
# ---------------------------------------------------------------------------


def test_rest_api(base_url):
    print("--- REST API ---")

    # Stats
    status, data = api("GET", "/api/v1/stats", base_url)
    test("GET /stats returns 200", status == 200)
    test("stats has total_memories", "total_memories" in data)

    # Create memory
    status, mem = api("POST", "/api/v1/memories", base_url, {
        "title": "Test Memory",
        "content": "Integration test content",
        "scope": "test/e2e",
        "tags": ["e2e", "automated"],
        "memory_type": "knowledge",
    })
    test("POST /memories returns 201", status == 201)
    test("memory has id", "id" in mem)
    mem_id = mem.get("id", "")

    # Get memory
    status, got = api("GET", f"/api/v1/memories/{mem_id}", base_url)
    test("GET /memories/:id returns 200", status == 200)
    test("memory title matches", got.get("title") == "Test Memory")
    test("memory tags match", set(got.get("tags", [])) == {"e2e", "automated"})

    # Search
    status, results = api("GET", "/api/v1/memories?q=integration+test&scope=test", base_url)
    test("GET /memories?q= returns 200", status == 200)
    test("search finds the memory", len(results) >= 1)

    # Update
    status, updated = api("PUT", f"/api/v1/memories/{mem_id}", base_url, {
        "title": "Updated Title",
        "tags": ["e2e", "updated"],
    })
    test("PUT /memories/:id returns 200", status == 200)
    test("title updated", updated.get("title") == "Updated Title")
    test("tags replaced", updated.get("tags") == ["e2e", "updated"])

    # Scopes
    status, scopes = api("GET", "/api/v1/scopes?prefix=test", base_url)
    test("GET /scopes returns 200", status == 200)
    test("scopes includes test/e2e", any(s["scope"] == "test/e2e" for s in scopes))

    # Tags
    status, tags = api("GET", "/api/v1/tags", base_url)
    test("GET /tags returns 200", status == 200)
    test("tags includes e2e", any(t["tag"] == "e2e" for t in tags))

    # Delete
    status, deleted = api("DELETE", f"/api/v1/memories/{mem_id}", base_url)
    test("DELETE /memories/:id returns 200", status == 200)
    test("delete confirmed", deleted.get("deleted") is True)

    # Verify deleted
    status, _ = api("GET", f"/api/v1/memories/{mem_id}", base_url)
    test("deleted memory returns 404", status == 404)


def test_rest_graph(base_url):
    print("\n--- REST API: Knowledge Graph ---")

    # Create entities
    status, results = api("POST", "/api/v1/graph/entities", base_url, {
        "scope": "test/e2e",
        "entities": [
            {"name": "ServiceA", "entity_type": "service", "observations": ["gRPC service"]},
            {"name": "ServiceB", "entity_type": "service", "observations": ["REST service"]},
        ],
    })
    test("POST /graph/entities returns 201", status == 201)
    test("2 entities created", len(results) == 2)
    test("ServiceA created", results[0].get("created") is True)

    # Create relation
    status, rels = api("POST", "/api/v1/graph/relations", base_url, {
        "scope": "test/e2e",
        "relations": [{"from": "ServiceA", "to": "ServiceB", "relation_type": "calls"}],
    })
    test("POST /graph/relations returns 201", status == 201)

    # Read graph
    status, graph = api("GET", "/api/v1/graph?scope=test/e2e", base_url)
    test("GET /graph returns 200", status == 200)
    test("graph has 2 entities", len(graph.get("entities", [])) == 2)
    test("graph has 1 relation", len(graph.get("relations", [])) == 1)

    # Get entity
    status, entity = api("GET", "/api/v1/graph/entities/ServiceA?scope=test/e2e", base_url)
    test("GET /graph/entities/:name returns 200", status == 200)
    test("entity has observations", len(entity.get("observations", [])) >= 1)

    # Add observation
    status, entity = api("POST", "/api/v1/graph/entities/ServiceA/observations?scope=test/e2e",
                         base_url, {"observations": ["New fact"]})
    test("POST observations returns 200", status == 200)
    test("observation added", len(entity.get("observations", [])) >= 2)

    # Search nodes
    status, results = api("GET", "/api/v1/graph/entities?q=gRPC&scope=test/e2e", base_url)
    test("GET /graph/entities?q= returns 200", status == 200)
    test("search finds ServiceA", len(results) >= 1)

    # Open nodes
    status, graph = api("GET", "/api/v1/graph/open?names=ServiceA&scope=test/e2e", base_url)
    test("GET /graph/open returns 200", status == 200)
    test("open includes neighbors", len(graph.get("entities", [])) >= 2)

    # Cleanup: delete entities (cascades)
    status, _ = api("DELETE", "/api/v1/graph/entities/ServiceA?scope=test/e2e", base_url)
    test("DELETE entity ServiceA", status == 200)
    status, _ = api("DELETE", "/api/v1/graph/entities/ServiceB?scope=test/e2e", base_url)
    test("DELETE entity ServiceB", status == 200)


def test_cli(base_url):
    print("\n--- CLI ---")
    env = {"STELE_URL": base_url}

    # Status
    rc, out, _ = cli("status", env_extra=env)
    test("stele status exits 0", rc == 0)
    test("status reports reachable", "reachable" in out.lower())

    # Store
    rc, out, _ = cli(
        "store", "--title", "CLI Test", "--content", "From CLI",
        "--scope", "test/cli", "--tags", "cli,automated",
        env_extra=env, json_output=True,
    )
    test("stele store exits 0", rc == 0)
    mem = json.loads(out) if rc == 0 else {}
    mem_id = mem.get("id", "")
    test("store returns memory with id", bool(mem_id))

    # Recall
    rc, out, _ = cli("recall", "CLI Test", "--scope", "test/cli", env_extra=env, json_output=True)
    test("stele recall exits 0", rc == 0)
    results = json.loads(out) if rc == 0 else []
    test("recall finds the memory", len(results) >= 1)

    # Get
    rc, out, _ = cli("get", mem_id, env_extra=env, json_output=True)
    test("stele get exits 0", rc == 0)

    # Update
    rc, out, _ = cli("update", mem_id, "--title", "Updated CLI", env_extra=env, json_output=True)
    test("stele update exits 0", rc == 0)
    updated = json.loads(out) if rc == 0 else {}
    test("update changed title", updated.get("title") == "Updated CLI")

    # Scopes
    rc, out, _ = cli("scopes", env_extra=env, json_output=True)
    test("stele scopes exits 0", rc == 0)

    # Tags
    rc, out, _ = cli("tags", env_extra=env, json_output=True)
    test("stele tags exits 0", rc == 0)

    # Stats
    rc, out, _ = cli("stats", env_extra=env, json_output=True)
    test("stele stats exits 0", rc == 0)

    # Forget
    rc, out, _ = cli("forget", mem_id, env_extra=env, json_output=True)
    test("stele forget exits 0", rc == 0)

    # Config path
    rc, out, _ = cli("config", "path")
    test("stele config path exits 0", rc == 0)
    test("config path is non-empty", bool(out.strip()))


def test_cli_graph(base_url):
    print("\n--- CLI: Knowledge Graph ---")
    env = {"STELE_URL": base_url}

    # Entities create
    rc, _, _ = cli(
        "graph", "entities", "create",
        "--name", "TestNode", "--entity-type", "test", "--scope", "test/cli",
        "--observations", "fact1,fact2",
        env_extra=env, json_output=True,
    )
    test("graph entities create exits 0", rc == 0)

    # Entities get
    rc, out, _ = cli("graph", "entities", "get", "TestNode", "--scope", "test/cli",
                      env_extra=env, json_output=True)
    test("graph entities get exits 0", rc == 0)

    # Graph read
    rc, out, _ = cli("graph", "read", "--scope", "test/cli", env_extra=env, json_output=True)
    test("graph read exits 0", rc == 0)

    # Observations add
    rc, _, _ = cli("graph", "observations", "add", "TestNode",
                    "--scope", "test/cli", "--observations", "fact3",
                    env_extra=env, json_output=True)
    test("graph observations add exits 0", rc == 0)

    # Graph search
    rc, out, _ = cli("graph", "search", "fact", "--scope", "test/cli", env_extra=env, json_output=True)
    test("graph search exits 0", rc == 0)

    # Cleanup
    rc, _, _ = cli("graph", "entities", "delete", "TestNode",
                    "--scope", "test/cli", env_extra=env, json_output=True)
    test("graph entities delete exits 0", rc == 0)


def test_mcp_proxy(base_url):
    print("\n--- MCP Stdio Proxy ---")

    # Initialize
    responses = mcp_call([
        {"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {
            "protocolVersion": "2025-03-26",
            "capabilities": {},
            "clientInfo": {"name": "e2e-test", "version": "0.1.0"},
        }},
    ], base_url)
    test("MCP initialize returns response", len(responses) >= 1)
    init_resp = responses[0] if responses else {}
    test("initialize has result", "result" in init_resp)
    test("protocol version matches",
         init_resp.get("result", {}).get("protocolVersion") == "2025-03-26")

    # Initialize + list tools
    responses = mcp_call([
        {"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {
            "protocolVersion": "2025-03-26",
            "capabilities": {},
            "clientInfo": {"name": "e2e-test", "version": "0.1.0"},
        }},
        {"jsonrpc": "2.0", "method": "notifications/initialized"},
        {"jsonrpc": "2.0", "id": 2, "method": "tools/list"},
    ], base_url)
    tools_resp = next((r for r in responses if r.get("id") == 2), None)
    test("tools/list returns response", tools_resp is not None)
    tools = tools_resp.get("result", {}).get("tools", []) if tools_resp else []
    test("17 tools listed", len(tools) == 17, f"got {len(tools)}")
    tool_names = {t["name"] for t in tools}
    test("store_memory in tools", "store_memory" in tool_names)
    test("recall_memories in tools", "recall_memories" in tool_names)
    test("create_entities in tools", "create_entities" in tool_names)

    # Tool call
    responses = mcp_call([
        {"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {
            "protocolVersion": "2025-03-26",
            "capabilities": {},
            "clientInfo": {"name": "e2e-test", "version": "0.1.0"},
        }},
        {"jsonrpc": "2.0", "method": "notifications/initialized"},
        {"jsonrpc": "2.0", "id": 3, "method": "tools/call", "params": {
            "name": "store_memory",
            "arguments": {
                "title": "MCP Test",
                "content": "Created via MCP proxy",
                "scope": "test/mcp",
                "tags": ["mcp", "e2e"],
            },
        }},
    ], base_url)
    call_resp = next((r for r in responses if r.get("id") == 3), None)
    test("tools/call returns response", call_resp is not None)
    if call_resp:
        content = call_resp.get("result", {}).get("content", [{}])
        text = content[0].get("text", "") if content else ""
        test("tool call returned memory", '"title"' in text and "MCP Test" in text)

    # Error handling: bad server
    responses = mcp_call([
        {"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {
            "protocolVersion": "2025-03-26",
            "capabilities": {},
            "clientInfo": {"name": "e2e-test", "version": "0.1.0"},
        }},
    ], "http://127.0.0.1:1")  # unreachable
    test("proxy returns JSON-RPC error on connection failure",
         len(responses) >= 1 and "error" in responses[0])


# ---------------------------------------------------------------------------
# Main
# ---------------------------------------------------------------------------


def main():
    global passed, failed

    no_build = "--no-build" in sys.argv

    if not no_build:
        build()

    # Verify binaries exist
    for binary, name in [(SERVER_BIN, "stele-server"), (CLI_BIN, "stele")]:
        if not os.path.isfile(binary):
            print(f"Binary not found: {binary}")
            print(f"Run 'cargo build -p {name}' first, or omit --no-build")
            sys.exit(1)

    # Create temp DB and find free port
    db_fd, db_path = tempfile.mkstemp(suffix=".db", prefix="stele-e2e-")
    os.close(db_fd)
    os.unlink(db_path)  # server creates it

    port = find_free_port()
    base_url = f"http://127.0.0.1:{port}"
    bind_addr = f"127.0.0.1:{port}"

    print(f"Starting server on {bind_addr} with DB {db_path}")
    server = subprocess.Popen(
        [SERVER_BIN, "--bind", bind_addr, "--db", db_path],
        stdout=subprocess.DEVNULL, stderr=subprocess.PIPE,
    )

    try:
        if not wait_for_server(base_url):
            print("Server failed to start within 10s")
            print(server.stderr.read().decode()[-1000:])
            sys.exit(1)

        print(f"Server running (pid {server.pid})\n")

        test_rest_api(base_url)
        test_rest_graph(base_url)
        test_cli(base_url)
        test_cli_graph(base_url)
        test_mcp_proxy(base_url)

    finally:
        server.send_signal(signal.SIGINT)
        try:
            server.wait(timeout=5)
        except subprocess.TimeoutExpired:
            server.kill()
            server.wait()

        # Cleanup temp files
        for f in [db_path, db_path + "-wal", db_path + "-shm"]:
            try:
                os.unlink(f)
            except FileNotFoundError:
                pass

    print(f"\n{'=' * 50}")
    print(f"Results: {passed} passed, {failed} failed")
    if errors:
        print(f"Failures: {', '.join(errors)}")
    print(f"{'=' * 50}")

    sys.exit(1 if failed > 0 else 0)


if __name__ == "__main__":
    main()
