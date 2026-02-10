---
description: |
  mcplug configuration system — config files, precedence, merging, env var
  expansion, editor imports, and transport types.
  Use when the user asks about: mcplug config, config precedence, editor imports,
  env var expansion, server config, mcplug.json, mcpServers, lifecycle modes.
---

# mcplug Configuration

## Config Precedence (6 levels, earlier wins)

When the same server name appears in multiple sources, the **first** source wins:

| Priority | Source | Path |
|----------|--------|------|
| 1 (highest) | `--config` CLI flag | user-specified path |
| 2 | `MCPLUG_CONFIG` env var | path from env var |
| 3 | Project-level | `./config/mcplug.json` |
| 4 | Home-level | `~/.mcplug/mcplug.json` or `~/.mcplug/mcplug.jsonc` |
| 5 | mcporter fallback | `~/.mcporter/mcporter.json[c]`, `./config/mcporter.json` |
| 6 (lowest) | Editor imports | Paths determined by `"imports"` array |

Source: `src/config/loader.rs` — `discover_config_files()` and `load_config()`.

## Config File Schema

Config files use JSON or JSONC (comments are stripped before parsing):

```jsonc
{
  // Server definitions
  "mcpServers": {
    "server-name": {
      "description": "Optional description",       // Optional<String>
      "baseUrl": "https://mcp.example.com/mcp",   // HTTP/SSE transport
      "command": "npx",                             // stdio transport
      "args": ["-y", "some-server"],               // stdio args
      "env": {"API_KEY": "${MY_KEY}"},             // env vars for child process
      "headers": {"Authorization": "Bearer tok"},  // HTTP headers
      "lifecycle": "keep-alive"                    // "keep-alive" | "ephemeral"
    }
  },
  // Import MCP configs from editors
  "imports": ["cursor", "claude-code", "vscode"]
}
```

## Transport Types

A server config must have either `baseUrl` (HTTP/SSE) or `command` (stdio). If both are present, `baseUrl` takes priority.

| Field | Transport | Protocol |
|-------|-----------|----------|
| `baseUrl` | HTTP/SSE | HTTP + Server-Sent Events for streaming |
| `command` + `args` | stdio | JSON-RPC over stdin/stdout of child process |

## Environment Variable Expansion

All string fields in server configs are expanded. Three syntaxes are supported:

| Syntax | Behavior | Example |
|--------|----------|---------|
| `${VAR}` | Replaced with env var value; **error if unset** | `${API_KEY}` |
| `${VAR:-fallback}` | Replaced with env var value, or fallback if unset/empty | `${API_KEY:-default}` |
| `$env:VAR` | Same as `${VAR}` (PowerShell-style) | `$env:API_KEY` |

Expansion applies to: `baseUrl`, `command`, `args`, `env` values, `headers` values.

A bare `$` not followed by `{` or `env:` is treated as a literal `$`.

Source: `src/config/env.rs` — `expand_env_vars()` and `expand_server_config()`.

## Editor Auto-Discovery

The `"imports"` array in config triggers auto-discovery of MCP servers from editor config files:

| Editor | Config Path |
|--------|-------------|
| `cursor` | `~/.cursor/mcp.json` |
| `claude-desktop` | `~/Library/Application Support/Claude/claude_desktop_config.json` (macOS) |
| `claude-code` | `~/.claude/.mcp.json` |
| `vscode` | `~/.vscode/mcp.json` |
| `windsurf` | `~/.windsurf/mcp.json` |
| `codex` | `~/.codex/mcp.json` |
| `opencode` | `~/.opencode/mcp.json` |

Editor configs are imported at the **lowest precedence** — they never override servers defined in mcplug's own config files.

Source: `src/config/editors.rs` — `editor_config_paths()` and `import_editor_configs()`.

## Lifecycle Modes

| Mode | Behavior |
|------|----------|
| `keep-alive` | Connection stays open; managed by daemon (`mcplug daemon`) |
| `ephemeral` | Connect on demand, disconnect after each operation |
| (unset) | Defaults to ephemeral behavior |

Lifecycle can be overridden via environment variables:
- `MCPLUG_KEEPALIVE=server_name` or `MCPLUG_KEEPALIVE=*` → forces keep-alive
- `MCPLUG_DISABLE_KEEPALIVE=server_name` or `MCPLUG_DISABLE_KEEPALIVE=*` → forces ephemeral

Source: `src/runtime.rs` — `effective_lifecycle()`.

## Environment Variables

| Variable | Purpose |
|----------|---------|
| `MCPLUG_CONFIG` | Override config file path |
| `MCPLUG_LOG_LEVEL` | Set log verbosity (uses `tracing` `EnvFilter`, default: `warn`) |
| `MCPLUG_OAUTH_TIMEOUT_MS` | OAuth flow timeout in milliseconds (default: 60000) |
| `MCPLUG_KEEPALIVE` | Force keep-alive lifecycle for a server or `*` for all |
| `MCPLUG_DISABLE_KEEPALIVE` | Force ephemeral lifecycle for a server or `*` for all |

## Key Source Files

- `src/config/loader.rs` — Config discovery, precedence, JSONC stripping, merging
- `src/config/types.rs` — `McplugConfig`, `ServerConfig`, `Lifecycle` structs
- `src/config/env.rs` — Environment variable expansion (3 syntaxes)
- `src/config/editors.rs` — Editor config paths and import logic (7 editors)
