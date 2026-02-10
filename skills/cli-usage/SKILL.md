---
description: |
  mcplug CLI commands, argument formats, output modes, and ad-hoc connections.
  Use when the user asks about: mcplug CLI, mcplug commands, argument formats,
  how to call a tool, mcplug list, mcplug call, output modes, typo detection.
---

# mcplug CLI Usage

## Commands

| Command | Description | Key Flags |
|---------|-------------|-----------|
| `mcplug list [server]` | List configured servers and their tools | `--json`, `--all-parameters`, `--http-url`, `--stdio` |
| `mcplug call <server.tool> [args...]` | Call an MCP tool | `--raw`, `--json`, `--output`, `--http-url`, `--stdio` |
| `mcplug auth <server>` | Complete OAuth login for a protected server | `--oauth-timeout` (env: `MCPLUG_OAUTH_TIMEOUT_MS`) |
| `mcplug daemon start\|stop\|restart\|status` | Manage persistent background servers | `start --log`, `start\|stop\|restart [server]` |
| `mcplug generate-cli <server>` | Generate a standalone CLI binary for a server | `--compile`, `--include-tools`, `--exclude-tools` |
| `mcplug emit-rs <server>` | Emit Rust type definitions and client wrappers | `--output <path>` |
| `mcplug config add\|show` | Manage server configuration | `add` is interactive, `show` displays merged config |

## Tool Reference Format

Tools are referenced as `server.tool` — the server name is everything before the first dot, the tool name is everything after (dots in tool names are preserved).

```
mcplug call firecrawl.scrape url:https://example.com
```

## Argument Formats (5 styles)

### 1. Colon-delimited
```
mcplug call server.tool key:value count:42 flag:true
```

### 2. Equals-delimited
```
mcplug call server.tool key=value count=42
```

### 3. Function-call with named args
```
mcplug call 'server.tool(key: "value", count: 42, active: true)'
```

### 4. Function-call with positional args
```
mcplug call 'server.tool("value", 42)'
```
Positional args are returned as a JSON array.

### 5. Mixed (colon and equals can be mixed freely)
```
mcplug call server.tool url:https://example.com depth=3 verbose:true
```

## Value Coercion Rules

Values are automatically coerced in this order:
1. **Quoted strings** (`"hello"` or `'hello'`) → stripped quotes, always string
2. **Booleans** (`true`/`false`, case-insensitive) → JSON boolean
3. **Null** (`null`) → JSON null
4. **Integers** (e.g., `42`, `-7`) → JSON number
5. **Floats** (e.g., `3.14`) → JSON number
6. **JSON objects/arrays** (e.g., `{"a":1}`, `[1,2,3]`) → parsed JSON
7. **Everything else** → JSON string

Source: `src/args.rs` — `coerce_value()` function.

## Output Modes

| Mode | Flag | Behavior |
|------|------|----------|
| **Pretty** | (default) | Colorized text for TTY; plain text for non-TTY |
| **Raw** | `--raw` | Unformatted content blocks only |
| **JSON** | `--json` | Machine-readable JSON to stdout |

Errors always go to stderr unless `--json` mode is active. Exit codes: `0` = success, `1` = error.

## Ad-hoc Connections

Both `list` and `call` support ad-hoc connections without requiring configuration:

```bash
# HTTP/SSE endpoint
mcplug list --http-url https://mcp.example.com/mcp
mcplug call --http-url https://mcp.example.com/mcp server.tool key:value

# Stdio server
mcplug list --stdio "npx -y some-mcp-server"
mcplug call --stdio "npx -y some-mcp-server" server.tool key:value
```

## Typo Detection

If you misspell a tool name, mcplug uses Levenshtein distance (threshold ≤ 2) to suggest the closest match. It only suggests when there's a single unambiguous match.

Source: `src/args.rs` — `suggest_tool()` function.

## Key Source Files

- `src/main.rs` — CLI entry point, clap command definitions
- `src/args.rs` — Argument parsing, value coercion, function-call syntax, typo detection
- `src/cli/call.rs` — `mcplug call` implementation
- `src/cli/list.rs` — `mcplug list` implementation
- `src/cli/output.rs` — Output formatting (TTY color, JSON, raw)
- `src/cli/connection.rs` — Ad-hoc connection helpers (`--http-url`, `--stdio`)
- `src/cli/config_cmd.rs` — `mcplug config add|show`
