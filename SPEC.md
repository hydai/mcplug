# mcplug Specification

A Rust toolkit for discovering, calling, and composing Model Context Protocol (MCP) servers — as a CLI tool and library crate.

## Intent

### Purpose

mcplug provides a native, dependency-free way to interact with MCP servers configured across development tools, replacing the need for a Node.js runtime.

### Users

| User | Goal |
|------|------|
| CLI developer | Call MCP tools from scripts and terminals without Node.js |
| Rust developer | Integrate MCP tool calling into Rust applications via library crate |
| AI-tool user | Discover and invoke MCP servers configured in Cursor, Claude, VS Code, etc. |

### Impacts

- Developers call MCP tools from CLI without installing Node.js or npx
- Rust applications embed MCP calling via `mcplug` crate dependency
- Existing mcporter users migrate by changing the binary name — config files are compatible

### Success Criteria

- All CLI commands from mcporter are supported: `list`, `call`, `auth`, `daemon`, `generate-cli`, `emit-rs`
- Reads mcporter-format config files and editor imports without modification
- Library crate exposes async API equivalent to mcporter's `callOnce`, `createRuntime`, `createServerProxy`
- stdio and HTTP/SSE transports connect to MCP servers correctly
- OAuth browser flow completes and caches tokens
- Daemon lifecycle manages stateful servers across invocations

### Non-goals

- WebSocket transport (future consideration, not in scope)
- GUI or TUI interface
- Acting as an MCP server (mcplug is a client only)
- Backward compatibility with mcporter's TypeScript programmatic API signatures

---

## Design

### System Boundary

| Type | Inside | Outside |
|------|--------|---------|
| Responsibility | Discover, connect, call, and manage MCP servers | Implementing MCP server logic |
| Interaction | Reads config files, spawns stdio processes, makes HTTP requests | Editor plugin integration, MCP server internals |
| Control | Config merging, transport lifecycle, token cache | MCP server availability, OAuth provider behavior |

### CLI Commands

#### `mcplug list`

List configured MCP servers and their tools.

| Variant | Behavior |
|---------|----------|
| `mcplug list` | Display all configured servers with connection status |
| `mcplug list <server>` | Display tools for a specific server as function signatures |
| `mcplug list --http-url <url>` | Query an ad-hoc HTTP endpoint |
| `mcplug list --stdio "<cmd>"` | Query an ad-hoc stdio server |
| `mcplug list --json` | Machine-readable output with status counts |

**Tool signature display:** Required parameters always shown. Optional parameters hidden unless `--all-parameters` flag is set or there are fewer than 5 required parameters.

**Timeout:** 30 seconds default. Override with `MCPLUG_LIST_TIMEOUT` environment variable.

#### `mcplug call`

Call an MCP tool.

| Variant | Behavior |
|---------|----------|
| `mcplug call <server>.<tool> [args]` | Call a tool with arguments |
| `mcplug <server>.<tool> [args]` | Shorthand — infers `call` verb |
| `mcplug call '<server>.<tool>(args)'` | Function-call syntax |

**Timeout:** 30 seconds default. Override with `MCPLUG_CALL_TIMEOUT` environment variable.

**Output flags:**

| Flag | Effect |
|------|--------|
| (default) | Pretty-printed, colorized for TTY |
| `--raw` | Unformatted output |
| `--json` | JSON output |
| `--output json\|raw` | Explicit output format |

#### `mcplug auth`

Complete OAuth login for a protected MCP server.

| Variant | Behavior |
|---------|----------|
| `mcplug auth <server>` | OAuth login for a configured server |
| `mcplug auth <url>` | OAuth login for an ad-hoc HTTP endpoint |

**Timeout:** 60 seconds default for browser handshake. Override with `MCPLUG_OAUTH_TIMEOUT_MS` or `--oauth-timeout <ms>`.

**Flow:**
1. Open browser to authorization URL
2. Listen on localhost callback
3. Exchange code for tokens
4. Cache tokens to `~/.mcplug/<server>/`

#### `mcplug daemon`

Manage persistent background servers.

| Subcommand | Behavior |
|------------|----------|
| `mcplug daemon start [server]` | Start daemon for keep-alive servers |
| `mcplug daemon stop [server]` | Stop running daemon |
| `mcplug daemon restart [server]` | Restart daemon |
| `mcplug daemon status` | Show daemon status for all servers |
| `mcplug daemon start --log` | Start with detailed logging enabled |

Daemons manage servers with `"lifecycle": "keep-alive"` in config. Ad-hoc servers are always ephemeral unless persisted.

#### `mcplug generate-cli`

Generate a standalone CLI binary for a specific MCP server.

| Variant | Behavior |
|---------|----------|
| `mcplug generate-cli <server>` | Generate Rust source for a standalone CLI |
| `mcplug generate-cli <server> --compile` | Generate and compile to binary |
| `mcplug generate-cli <server> --include-tools <list>` | Include only specified tools |
| `mcplug generate-cli <server> --exclude-tools <list>` | Exclude specified tools |

Output: Rust source file(s) implementing a CLI that calls the specified server's tools directly.

#### `mcplug emit-rs`

Emit Rust type definitions and client wrappers for an MCP server.

| Variant | Behavior |
|---------|----------|
| `mcplug emit-rs <server>` | Print Rust types to stdout |
| `mcplug emit-rs <server> --output <path>` | Write to file |

Output: Rust structs for tool input/output schemas, plus typed wrapper functions.

#### `mcplug config`

Manage server configuration.

| Subcommand | Behavior |
|------------|----------|
| `mcplug config add` | Interactive: add a new server definition |
| `mcplug config show` | Display merged config with source annotations |

### Argument Parsing

mcplug accepts tool arguments in multiple formats, normalized to a key-value map before invocation.

| Format | Example | Notes |
|--------|---------|-------|
| Colon-delimited | `key:value` | Shell-friendly |
| Equals | `key=value` | Alternative syntax |
| Colon with space | `key: value` | Within quoted function-call strings |
| Function-call | `'tool(key: "value")'` | Supports nested objects and arrays |
| Positional | `'tool("value")'` | Maps to schema-required fields in order |

**Typo correction:** When a tool name doesn't match exactly, compute edit distance against known tools. If a single match is within threshold (Levenshtein distance <= 2), suggest it with "Did you mean...?". Do not auto-execute.

### Configuration

#### File Locations (precedence order, highest first)

1. `--config <path>` CLI flag
2. `MCPLUG_CONFIG` environment variable
3. `./config/mcplug.json` (project-level)
4. `~/.mcplug/mcplug.json` or `~/.mcplug/mcplug.jsonc` (home-level)

#### Compatibility

mcplug also reads mcporter config files as fallback:
- `~/.mcporter/mcporter.json[c]`
- `./config/mcporter.json`

If both mcplug and mcporter configs exist, mcplug config takes precedence.

#### Config Schema

```jsonc
{
  "mcpServers": {
    "<server-name>": {
      "description": "Human-readable description",
      // HTTP transport
      "baseUrl": "https://mcp.example.com/mcp",
      // stdio transport
      "command": "executable",
      "args": ["arg1", "arg2"],
      // Shared
      "env": { "KEY": "value" },
      "headers": { "Authorization": "$env:API_KEY" },
      "lifecycle": "keep-alive" | "ephemeral"
    }
  },
  "imports": ["cursor", "claude-code", "claude-desktop", "codex", "windsurf", "opencode", "vscode"]
}
```

#### Environment Variable Expansion

| Syntax | Behavior |
|--------|----------|
| `${VAR}` | Replaced with env var value; error if unset |
| `${VAR:-fallback}` | Replaced with env var value, or fallback if unset |
| `$env:VAR` | Same as `${VAR}` — alternative syntax for headers |

Expansion applies to: `env` values, `headers` values, `baseUrl`, `command`, `args`.

#### Editor Import Locations

| Editor | Config Path |
|--------|------------|
| Cursor | `~/.cursor/mcp.json` |
| Claude Desktop | `~/Library/Application Support/Claude/claude_desktop_config.json` (macOS), `%APPDATA%/Claude/claude_desktop_config.json` (Windows) |
| Claude Code | `~/.claude/.mcp.json` and project `.mcp.json` |
| VS Code | `~/.vscode/mcp.json` |
| Windsurf | `~/.windsurf/mcp.json` |
| Codex | `~/.codex/mcp.json` |
| OpenCode | `~/.opencode/mcp.json` |

Editor configs are merged after mcplug/mcporter configs. If the same server name appears in multiple sources, earlier sources win.

### Transports

#### stdio

- Spawn a child process with specified `command` and `args`
- Communicate via JSON-RPC over stdin/stdout
- Process inherits calling shell environment, merged with `env` from config
- Working directory: directory containing the config file that defined the server
- Override with `--cwd <path>` or `--root <path>`

**Ad-hoc:** `mcplug list --stdio "npx -y some-mcp-server"` or `mcplug call --stdio "..." server.tool args`

#### HTTP/SSE

- Connect to `baseUrl` via HTTP
- Use Server-Sent Events (SSE) for streaming responses
- Include `headers` from config in all requests
- Cleartext HTTP requires `--allow-http` flag

**Ad-hoc:** `mcplug list --http-url https://mcp.example.com/mcp`

#### Transport Trait (Library)

```rust
#[async_trait]
pub trait McpTransport: Send + Sync {
    async fn initialize(&mut self) -> Result<ServerInfo>;
    async fn list_tools(&self) -> Result<Vec<ToolDefinition>>;
    async fn call_tool(&self, name: &str, args: serde_json::Value) -> Result<CallResult>;
    async fn close(&mut self) -> Result<()>;
}
```

Both `StdioTransport` and `HttpSseTransport` implement this trait.

### OAuth

#### Flow

1. Discover OAuth metadata from MCP server's `/.well-known/oauth-authorization-server`
2. Generate PKCE code verifier and challenge
3. Open system browser to authorization URL with redirect to `http://localhost:<port>/callback`
4. Listen on localhost for callback
5. Exchange authorization code for access/refresh tokens
6. Cache tokens to `~/.mcplug/<server-name>/tokens.json`

#### Token Lifecycle

| Event | Behavior |
|-------|----------|
| Token valid | Use cached access token |
| Token expired, refresh token valid | Refresh silently |
| Token expired, no refresh token | Re-prompt browser login |
| Token file missing | Prompt browser login |

#### Server State

Servers requiring OAuth that lack cached tokens report status `auth` in `mcplug list --json`.

### Daemon Management

#### Architecture

The daemon is a background process that keeps stdio MCP servers alive between CLI invocations.

| Concept | Behavior |
|---------|----------|
| Start | Spawn background process managing all `keep-alive` servers |
| Communication | CLI connects to daemon via Unix domain socket (`~/.mcplug/daemon.sock`) |
| Stop | Send shutdown signal; daemon gracefully terminates child processes |
| Status | Report PID, uptime, managed server count, per-server connection state |

#### Lifecycle Modes

| Mode | Behavior |
|------|----------|
| `keep-alive` | Managed by daemon; persists between CLI invocations |
| `ephemeral` | Spawned per-invocation; terminated after call completes |

Default: `ephemeral` unless `"lifecycle": "keep-alive"` is set in config.

Override per-server:
- `MCPLUG_KEEPALIVE=<server>` — force keep-alive for a server
- `MCPLUG_DISABLE_KEEPALIVE=<server>` — force ephemeral for a server

### Library API

The `mcplug` crate exposes an async API.

#### One-shot Call

```rust
use mcplug::call_once;

let result = call_once("firecrawl", "crawl", json!({"url": "https://example.com"})).await?;
println!("{}", result.text());
```

#### Runtime with Connection Pooling

```rust
use mcplug::Runtime;

let runtime = Runtime::from_config().await?;
let result = runtime.call_tool("context7", "resolve-library-id", json!({"libraryName": "react"})).await?;
runtime.close().await?;
```

#### Typed Server Proxy

```rust
use mcplug::ServerProxy;

let runtime = Runtime::from_config().await?;
let chrome = ServerProxy::new(&runtime, "chrome-devtools");
let snapshot = chrome.call("takeSnapshot", json!({})).await?;
println!("{}", snapshot.text());
```

#### Result Helpers

`CallResult` provides:

| Method | Return |
|--------|--------|
| `.text()` | Plain text extraction from content blocks |
| `.json::<T>()` | Deserialize content as type `T` |
| `.markdown()` | Markdown-formatted content |
| `.content()` | Raw content blocks |
| `.raw()` | Full MCP response envelope |

### Code Generation

#### `emit-rs`

For a given MCP server, generates:
- Rust structs for each tool's input parameters (from JSON Schema)
- Rust structs for each tool's output (when schema available)
- A typed client struct with one method per tool
- `serde::Serialize` / `serde::Deserialize` derives on all generated types

#### `generate-cli`

For a given MCP server, generates:
- A complete `main.rs` with `clap` CLI argument definitions per tool
- Subcommand per tool with typed arguments
- Connection setup and invocation logic
- Optional: compile with `--compile` flag using `cargo build`

### Error Handling

#### CLI Errors

| Scenario | Behavior |
|----------|----------|
| Unknown server | Exit 1, print "Server '<name>' not found. Available: ..." |
| Unknown tool | Exit 1, print "Tool '<name>' not found on <server>." + typo suggestion if within edit distance |
| Connection refused | Exit 1, print "Cannot connect to <server>: <reason>" |
| Timeout | Exit 1, print "Timeout after <N>s calling <server>.<tool>" |
| Missing required arg | Exit 1, print "Missing required argument: <name>" |
| Invalid arg format | Exit 1, print "Cannot parse arguments: <detail>" |
| OAuth required | Exit 1, print "Server '<name>' requires authentication. Run: mcplug auth <name>" |
| Config parse error | Exit 1, print "Error in config <path>: <detail>" |
| Env var unset (no fallback) | Exit 1, print "Environment variable '<name>' is not set (referenced in <path>)" |

#### Structured Error Output

When `--json` or `--output json` is active, errors are emitted as:

```json
{
  "error": {
    "server": "server-name",
    "tool": "tool-name",
    "message": "human-readable description",
    "code": "connection_refused | timeout | auth_required | not_found | parse_error | config_error"
  }
}
```

Exit code is always non-zero (1) for errors.

#### Library Errors

The library uses a typed error enum:

```rust
pub enum McplugError {
    ServerNotFound(String),
    ToolNotFound { server: String, tool: String },
    ConnectionFailed { server: String, source: Box<dyn std::error::Error + Send + Sync> },
    Timeout { server: String, tool: Option<String>, duration: Duration },
    AuthRequired(String),
    ConfigError { path: PathBuf, detail: String },
    TransportError(Box<dyn std::error::Error + Send + Sync>),
    ProtocolError(String),
}
```

Implements `std::error::Error` and `Display`.

### Ad-hoc Connections

| Flag | Behavior |
|------|----------|
| `--http-url <url>` | Connect to an HTTP/SSE endpoint without config |
| `--stdio "<cmd>"` | Spawn a stdio server without config |
| `--name <name>` | Assign a name to the ad-hoc server (default: derived from URL/command) |
| `--persist <path>` | Save the ad-hoc definition to a config file |
| `--allow-http` | Permit cleartext HTTP connections |
| `--env KEY=value` | Inject environment variables for stdio servers |

Ad-hoc connections are ephemeral by default. Use `--persist` to save for future use.

---

## Consistency

### Terminology

| Term | Meaning |
|------|---------|
| Server | A configured MCP server (named entry in config) |
| Tool | A callable function exposed by an MCP server |
| Transport | Communication mechanism (stdio or HTTP/SSE) |
| Runtime | Long-lived connection manager with pooling |
| Daemon | Background process keeping stdio servers alive |
| Keep-alive | Server lifecycle managed by daemon |
| Ephemeral | Server lifecycle limited to single invocation |

### Patterns

| Pattern | Rule |
|---------|------|
| Config merging | Later sources never override earlier sources for the same server name |
| Transport abstraction | All transports implement the same trait; callers are transport-agnostic |
| Timeout | Every network/process operation has a configurable timeout with a sensible default |
| Env var naming | All mcplug env vars prefixed with `MCPLUG_` |
| Structured output | `--json` flag on any command produces machine-readable JSON to stdout |
| Exit codes | 0 = success, 1 = error. No other exit codes. |
| Ad-hoc pattern | Any configured operation can be performed ad-hoc with `--http-url` or `--stdio` flags |

### Output Conventions

| Context | Format |
|---------|--------|
| TTY stdout | Colorized, human-readable |
| Non-TTY stdout | Plain text, no colors |
| `--json` | JSON to stdout |
| `--raw` | Unformatted MCP response content |
| Errors | stderr (human-readable) or structured JSON to stdout when `--json` |
| Logs | stderr, controlled by `MCPLUG_LOG_LEVEL` (debug, info, warn, error) |

### Environment Variables

| Variable | Default | Purpose |
|----------|---------|---------|
| `MCPLUG_CONFIG` | (none) | Override config file path |
| `MCPLUG_LIST_TIMEOUT` | 30000 | List operation timeout (ms) |
| `MCPLUG_CALL_TIMEOUT` | 30000 | Call operation timeout (ms) |
| `MCPLUG_OAUTH_TIMEOUT_MS` | 60000 | OAuth browser handshake timeout (ms) |
| `MCPLUG_LOG_LEVEL` | warn | Logging verbosity |
| `MCPLUG_KEEPALIVE` | (none) | Force keep-alive for named server |
| `MCPLUG_DISABLE_KEEPALIVE` | (none) | Force ephemeral for named server |
| `MCPLUG_DEBUG_HANG` | (none) | Enable hang debugging diagnostics |
