# Tests

## End-to-End Integration Tests

`tests/e2e_test.py` is a self-contained Python test suite that exercises the full Stele stack: server, REST API, CLI, and MCP stdio proxy.

### Requirements

- Python 3.8+ (stdlib only, no pip packages)
- Rust toolchain (for building binaries)

### Usage

```bash
# Build and test (from repo root)
python tests/e2e_test.py

# Skip build (use existing binaries)
python tests/e2e_test.py --no-build
```

### How It Works

1. Builds `stele-server` (headless) and `stele` (CLI) in debug mode
2. Starts the server on a random free port with a temporary SQLite database
3. Runs all test suites against the live server
4. Stops the server and cleans up temporary files
5. Exits with code 0 (all pass) or 1 (any failure)

### Test Suites

| Suite                       | Tests | What It Covers                                                                            |
| --------------------------- | ----- | ----------------------------------------------------------------------------------------- |
| **REST API: Memories**      | 19    | CRUD lifecycle, FTS search, scopes, tags, stats, 404 on deleted                           |
| **REST API: Knowledge Graph** | 16  | Entity create (idempotent), relations, graph read, entity search/open, observations, cascade delete |
| **CLI Commands**            | 15    | store, recall, get, update, forget, scopes, tags, stats, status, config path              |
| **CLI: Graph Commands**     | 6     | entities create/get/delete, observations add, graph read, graph search                    |
| **MCP Stdio Proxy**         | 11    | initialize, protocol version, tools/list (17 tools), tool call, error on unreachable server |
| **Total**                   | **68** |                                                                                          |

### CI

The e2e tests run automatically in GitHub Actions CI (see `.github/workflows/ci.yml`, `e2e` job). The CI job builds the binaries first, then runs the test script with `--no-build`.
