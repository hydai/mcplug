# mcplug

Rust toolkit for discovering, calling, and composing MCP (Model Context Protocol) servers. Dual-purpose: CLI binary + library crate.

## Build & Test

```sh
cargo build              # debug build
cargo build --release    # release build
cargo test               # run all tests
cargo test -- --nocapture # run tests with output
cargo clippy             # lint
```

## Architecture

```
src/
├── main.rs              # CLI entry point (clap-based)
├── lib.rs               # Library crate root, re-exports public API
├── transport.rs         # McpTransport trait (async_trait)
├── runtime.rs           # Runtime — connection pooling, config-based dispatch
├── server_proxy.rs      # ServerProxy — typed wrapper around Runtime
├── error.rs             # McplugError enum (thiserror)
├── types.rs             # Shared types: CallResult, ToolDefinition, ServerInfo, ContentBlock
├── args.rs              # Argument parsing (colon, equals, function-call, positional)
├── cli/                 # CLI command implementations
│   ├── list.rs          # `mcplug list`
│   ├── call.rs          # `mcplug call`
│   ├── config_cmd.rs    # `mcplug config add|show`
│   ├── connection.rs    # Ad-hoc connection helpers (--http-url, --stdio)
│   └── output.rs        # Output formatting (TTY color, JSON, raw)
├── config/              # Configuration loading
│   ├── loader.rs        # Multi-source config merging (precedence-based)
│   ├── types.rs         # McplugConfig, ServerConfig structs
│   ├── env.rs           # Environment variable expansion (${VAR}, ${VAR:-fallback}, $env:VAR)
│   └── editors.rs       # Editor config import (Cursor, Claude, VS Code, etc.)
├── transports/          # Transport implementations
│   ├── stdio.rs         # StdioTransport — child process over stdin/stdout
│   ├── http_sse.rs      # HttpSseTransport — HTTP + Server-Sent Events
│   └── jsonrpc.rs       # JSON-RPC message types
├── oauth/               # OAuth browser flow
│   ├── flow.rs          # Full OAuth orchestration
│   ├── discovery.rs     # .well-known/oauth-authorization-server discovery
│   ├── pkce.rs          # PKCE code verifier/challenge generation
│   ├── callback.rs      # Localhost callback listener
│   ├── token.rs         # Token types
│   └── cache.rs         # Token file caching (~/.mcplug/<server>/tokens.json)
├── codegen/             # Code generation
│   ├── emit_rs.rs       # `mcplug emit-rs` — Rust type generation from JSON Schema
│   └── generate_cli.rs  # `mcplug generate-cli` — standalone CLI generation
└── daemon/              # Daemon management
    └── manager.rs       # Start/stop/restart/status for keep-alive servers
```

## Key Patterns

- **Transport abstraction**: `McpTransport` trait in `transport.rs` — both `StdioTransport` and `HttpSseTransport` implement it. All callers are transport-agnostic.
- **Config merging**: configs load from multiple sources with precedence (CLI flag > env var > project > home > editor imports). Earlier sources win on name collisions.
- **mcporter compatibility**: reads mcporter config files as fallback. Config format is compatible.
- **Error handling**: `McplugError` enum with `thiserror` derives. Each variant has a `.code()` for structured JSON output. Errors go to stderr unless `--json` mode.
- **Async runtime**: tokio with full features. All transport operations are async.
- **Env var expansion**: supports `${VAR}`, `${VAR:-fallback}`, and `$env:VAR` syntax in config values.

## Conventions

- Exit codes: 0 = success, 1 = error. No other exit codes.
- Environment variables are prefixed with `MCPLUG_`.
- Logging goes to stderr via `tracing`, controlled by `MCPLUG_LOG_LEVEL`.
- TTY output is colorized; non-TTY is plain text.
- `--json` flag on any command produces machine-readable JSON to stdout.
- All timeouts are configurable with sensible defaults (30s list/call, 60s OAuth).

## Specification

See `SPEC.md` for the complete specification including all CLI commands, config schema, transport details, OAuth flow, daemon architecture, and library API.
