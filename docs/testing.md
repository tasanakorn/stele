# Testing

Stele uses end-to-end integration tests that exercise the full stack against a live server. There are no unit tests yet.

## Running Tests

```bash
# Build and test (from repo root)
python tests/e2e_test.py

# Skip build (use existing binaries in target/debug/)
python tests/e2e_test.py --no-build
```

Requirements: Python 3.8+ (stdlib only). The script builds the binaries, starts a headless server on a random port with a temporary database, runs all tests, then cleans up.

## Test Coverage

### REST API: Memories (19 tests)

| Test                           | Method | Endpoint                | Validates                                  |
| ------------------------------ | ------ | ----------------------- | ------------------------------------------ |
| GET /stats returns 200         | GET    | /api/v1/stats           | Server responds, stats structure present   |
| stats has total_memories       | GET    | /api/v1/stats           | Response includes total_memories field     |
| POST /memories returns 201     | POST   | /api/v1/memories        | Memory creation with all fields            |
| memory has id                  | POST   | /api/v1/memories        | ULID is generated and returned             |
| GET /memories/:id returns 200  | GET    | /api/v1/memories/{id}   | Fetch by ID works                          |
| memory title matches           | GET    | /api/v1/memories/{id}   | Title persisted correctly                  |
| memory tags match              | GET    | /api/v1/memories/{id}   | Tags persisted and returned                |
| GET /memories?q= returns 200   | GET    | /api/v1/memories        | FTS search endpoint works                  |
| search finds the memory        | GET    | /api/v1/memories        | FTS search returns matching result         |
| PUT /memories/:id returns 200  | PUT    | /api/v1/memories/{id}   | Partial update succeeds                    |
| title updated                  | PUT    | /api/v1/memories/{id}   | Title field changed                        |
| tags replaced                  | PUT    | /api/v1/memories/{id}   | Tags are fully replaced (not merged)       |
| GET /scopes returns 200        | GET    | /api/v1/scopes          | Scope listing works                        |
| scopes includes test/e2e       | GET    | /api/v1/scopes          | Created memory's scope appears             |
| GET /tags returns 200          | GET    | /api/v1/tags            | Tag listing works                          |
| tags includes e2e              | GET    | /api/v1/tags            | Created memory's tag appears               |
| DELETE /memories/:id returns 200 | DELETE | /api/v1/memories/{id} | Deletion succeeds                          |
| delete confirmed               | DELETE | /api/v1/memories/{id}   | Response has `deleted: true`               |
| deleted memory returns 404     | GET    | /api/v1/memories/{id}   | Deleted memory is gone                     |

### REST API: Knowledge Graph (16 tests)

| Test                            | Method | Endpoint                                  | Validates                                  |
| ------------------------------- | ------ | ----------------------------------------- | ------------------------------------------ |
| POST /graph/entities returns 201 | POST  | /api/v1/graph/entities                    | Batch entity creation                      |
| 2 entities created              | POST   | /api/v1/graph/entities                    | Correct count in response                  |
| ServiceA created                | POST   | /api/v1/graph/entities                    | `created: true` flag                       |
| POST /graph/relations returns 201 | POST | /api/v1/graph/relations                   | Relation creation between entities         |
| GET /graph returns 200          | GET    | /api/v1/graph                             | Full graph read                            |
| graph has 2 entities            | GET    | /api/v1/graph                             | Entity count matches                       |
| graph has 1 relation            | GET    | /api/v1/graph                             | Relation count matches                     |
| GET /graph/entities/:name returns 200 | GET | /api/v1/graph/entities/{name}          | Entity lookup by name + scope              |
| entity has observations         | GET    | /api/v1/graph/entities/{name}             | Observations attached to entity            |
| POST observations returns 200   | POST   | /api/v1/graph/entities/{name}/observations | Append observations                       |
| observation added               | POST   | /api/v1/graph/entities/{name}/observations | Observation count increased                |
| GET /graph/entities?q= returns 200 | GET | /api/v1/graph/entities                    | FTS entity search                          |
| search finds ServiceA           | GET    | /api/v1/graph/entities                    | FTS returns matching entity                |
| GET /graph/open returns 200     | GET    | /api/v1/graph/open                        | Open nodes with neighbors                  |
| open includes neighbors         | GET    | /api/v1/graph/open                        | Both entities returned (via relation)      |
| DELETE entity ServiceA          | DELETE | /api/v1/graph/entities/{name}             | Entity deletion cascades                   |
| DELETE entity ServiceB          | DELETE | /api/v1/graph/entities/{name}             | Second entity cleanup                      |

### CLI Commands (15 tests)

| Test                           | Command           | Validates                                  |
| ------------------------------ | ----------------- | ------------------------------------------ |
| stele status exits 0           | `stele status`    | Health check succeeds against live server  |
| status reports reachable       | `stele status`    | Output contains "reachable"                |
| stele store exits 0            | `stele store`     | Memory creation via CLI                    |
| store returns memory with id   | `stele store`     | JSON output contains `id` field            |
| stele recall exits 0           | `stele recall`    | FTS search via CLI                         |
| recall finds the memory        | `stele recall`    | At least 1 result returned                 |
| stele get exits 0              | `stele get`       | Fetch by ID via CLI                        |
| stele update exits 0           | `stele update`    | Partial update via CLI                     |
| update changed title           | `stele update`    | Title field changed in JSON output         |
| stele scopes exits 0           | `stele scopes`    | Scope listing via CLI                      |
| stele tags exits 0             | `stele tags`      | Tag listing via CLI                        |
| stele stats exits 0            | `stele stats`     | Stats via CLI                              |
| stele forget exits 0           | `stele forget`    | Deletion via CLI                           |
| stele config path exits 0      | `stele config path` | Config path command works                |
| config path is non-empty       | `stele config path` | Outputs a file path                      |

All CLI tests use `--json` output and `STELE_URL` env var to target the test server.

### CLI: Knowledge Graph (6 tests)

| Test                           | Command                        | Validates                          |
| ------------------------------ | ------------------------------ | ---------------------------------- |
| graph entities create exits 0  | `stele graph entities create`  | Entity creation via CLI            |
| graph entities get exits 0     | `stele graph entities get`     | Entity fetch via CLI               |
| graph read exits 0             | `stele graph read`             | Full graph read via CLI            |
| graph observations add exits 0 | `stele graph observations add` | Observation append via CLI         |
| graph search exits 0           | `stele graph search`           | FTS entity search via CLI          |
| graph entities delete exits 0  | `stele graph entities delete`  | Entity deletion via CLI            |

### MCP Stdio Proxy (11 tests)

| Test                           | MCP Method              | Validates                                    |
| ------------------------------ | ----------------------- | -------------------------------------------- |
| MCP initialize returns response | `initialize`           | Proxy forwards request and returns response  |
| initialize has result          | `initialize`            | Response contains `result` (not error)       |
| protocol version matches       | `initialize`            | Server negotiates `2025-03-26`               |
| tools/list returns response    | `tools/list`            | Multi-message session works (session ID tracked) |
| 17 tools listed                | `tools/list`            | All MCP tools are registered                 |
| store_memory in tools          | `tools/list`            | Flat memory tools present                    |
| recall_memories in tools       | `tools/list`            | Search tool present                          |
| create_entities in tools       | `tools/list`            | Knowledge graph tools present                |
| tools/call returns response    | `tools/call`            | Tool execution through proxy works           |
| tool call returned memory      | `tools/call`            | `store_memory` creates and returns a memory  |
| proxy returns JSON-RPC error   | `initialize` (bad URL)  | Connection failure returns JSON-RPC error, not crash |

The MCP tests validate the full stdio proxy pipeline: stdin JSON-RPC -> HTTP POST -> SSE parse -> stdout JSON-RPC. Each test starts a fresh session (initialize + notifications/initialized) to verify session ID tracking.

## CI Integration

The e2e tests run in GitHub Actions CI as the `e2e` job in `.github/workflows/ci.yml`. The job:

1. Checks out the code
2. Installs the Rust toolchain with caching
3. Builds `stele-server` (headless) and `stele-cli`
4. Runs `python3 tests/e2e_test.py --no-build`

## What Is Not Tested

- Desktop mode (tray icon, settings dialog) -- requires macOS GUI
- Auth middleware (`X-Stele-Key`) -- server-side auth not yet implemented
- MCP stdio proxy SSE keep-alive and long-lived connections
- Concurrent access / multiple simultaneous clients
- Large payloads / performance / stress testing
- The `bootstrap_project` MCP tool output content
- CLI `stele mcp` with `--profile` flag (tested manually)
- CLI `stele config init` and `stele config show` (would write to user's home directory)
