# mcplug

A Rust toolkit for discovering, calling, and composing [Model Context Protocol (MCP)](https://modelcontextprotocol.io/) servers — as a CLI tool and library crate.

No Node.js required. No npx. Just a single native binary.

## Features

- **Discover** MCP servers configured in Cursor, Claude Desktop, Claude Code, VS Code, Windsurf, Codex, and OpenCode
- **Call** any MCP tool from the command line with flexible argument formats
- **OAuth** browser-based authentication for protected MCP servers
- **Daemon** mode for persistent background server connections
- **Code generation** — emit Rust types or standalone CLI binaries from MCP server schemas
- **Library crate** — embed MCP tool calling in your own Rust applications

## Installation

```sh
cargo install --path .
```

Or build from source:

```sh
cargo build --release
```

## Quick Start

### List configured servers

```sh
mcplug list
```

### List tools on a specific server

```sh
mcplug list firecrawl
```

### Call a tool

```sh
mcplug call firecrawl.crawl url:https://example.com
```

Shorthand (infers `call`):

```sh
mcplug firecrawl.crawl url:https://example.com
```

Function-call syntax:

```sh
mcplug call 'firecrawl.crawl(url: "https://example.com")'
```

### Ad-hoc connections (no config needed)

```sh
# HTTP/SSE endpoint
mcplug list --http-url https://mcp.example.com/mcp

# stdio server
mcplug list --stdio "npx -y some-mcp-server"
```

## Configuration

mcplug reads config from these locations (highest precedence first):

1. `--config <path>` CLI flag
2. `MCPLUG_CONFIG` environment variable
3. `./config/mcplug.json` (project-level)
4. `~/.mcplug/mcplug.json` (home-level)

It also reads mcporter config files as a fallback for migration.

### Config format

```jsonc
{
  "mcpServers": {
    "firecrawl": {
      "baseUrl": "https://mcp.firecrawl.dev/mcp",
      "headers": { "Authorization": "$env:FIRECRAWL_API_KEY" }
    },
    "local-tool": {
      "command": "my-mcp-server",
      "args": ["--port", "3000"]
    }
  },
  "imports": ["cursor", "claude-code", "claude-desktop"]
}
```

### Editor imports

mcplug auto-discovers MCP servers configured in your editors:

| Editor | Config Path |
|--------|------------|
| Cursor | `~/.cursor/mcp.json` |
| Claude Desktop | `~/Library/Application Support/Claude/claude_desktop_config.json` |
| Claude Code | `~/.claude/.mcp.json` |
| VS Code | `~/.vscode/mcp.json` |
| Windsurf | `~/.windsurf/mcp.json` |
| Codex | `~/.codex/mcp.json` |
| OpenCode | `~/.opencode/mcp.json` |

## CLI Commands

| Command | Description |
|---------|-------------|
| `mcplug list [server]` | List servers or tools on a server |
| `mcplug call <server.tool> [args]` | Call an MCP tool |
| `mcplug auth <server>` | OAuth login for a protected server |
| `mcplug daemon start\|stop\|restart\|status` | Manage persistent background servers |
| `mcplug generate-cli <server>` | Generate a standalone CLI binary |
| `mcplug emit-rs <server>` | Emit Rust type definitions |
| `mcplug config add\|show` | Manage configuration |

Use `--json` on any command for machine-readable output.

## Library Usage

Add to your `Cargo.toml`:

```toml
[dependencies]
mcplug = { path = "." }
```

### One-shot call

```rust
use mcplug::call_once;

let result = call_once("firecrawl", "crawl", json!({"url": "https://example.com"})).await?;
println!("{}", result.text());
```

### Runtime with connection pooling

```rust
use mcplug::Runtime;

let runtime = Runtime::from_config().await?;
let result = runtime.call_tool("context7", "resolve-library-id", json!({"libraryName": "react"})).await?;
runtime.close().await?;
```

### Typed server proxy

```rust
use mcplug::{Runtime, ServerProxy};

let runtime = Runtime::from_config().await?;
let chrome = ServerProxy::new(&runtime, "chrome-devtools");
let snapshot = chrome.call("takeSnapshot", json!({})).await?;
```

## Testing

```sh
cargo test               # run all tests (unit + integration)
cargo test --lib         # unit tests only
cargo test --test cli_integration  # specific integration suite
```

304 tests covering all modules: argument parsing, config loading/merging, CLI commands, OAuth flow, transports (stdio + HTTP), runtime connection pooling, daemon management, code generation, and error handling.

Integration tests use a mock MCP server binary (`tests/fixtures/mock_mcp_server.rs`) that supports 5 tools over stdio. CLI tests use `assert_cmd` for end-to-end binary verification.

## Environment Variables

| Variable | Default | Purpose |
|----------|---------|---------|
| `MCPLUG_CONFIG` | — | Override config file path |
| `MCPLUG_LIST_TIMEOUT` | 30000 | List timeout (ms) |
| `MCPLUG_CALL_TIMEOUT` | 30000 | Call timeout (ms) |
| `MCPLUG_OAUTH_TIMEOUT_MS` | 60000 | OAuth timeout (ms) |
| `MCPLUG_LOG_LEVEL` | warn | Logging verbosity |

## Claude Code Plugin

mcplug ships with a [Claude Code](https://claude.com/claude-code) plugin that provides contextual help when working on or with the project.

```sh
claude --plugin-dir /path/to/mcplug
```

This enables:
- **Skills** — contextual knowledge about CLI commands, configuration, and library API (activated automatically by relevant questions)
- **Commands** — `/mcplug:quickstart` for first-run setup, `/mcplug:troubleshoot` for diagnosing issues
- **Agent** — `mcplug-guide` answers questions like "how do I use mcplug" by consulting skills and source code

## Acknowledgements

mcplug is inspired by [mcporter](https://github.com/steipete/mcporter), a Node.js-based MCP tool runner. mcplug aims to bring the same capabilities to the Rust ecosystem as a single native binary with no Node.js dependency.

## License

MIT
