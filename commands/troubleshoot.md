---
name: troubleshoot
description: Diagnose mcplug connection, config, auth, timeout, and daemon issues
allowed-tools:
  - Read
  - Glob
  - Grep
  - Bash
---

You are diagnosing mcplug issues. Gather information systematically before suggesting fixes.

## Diagnostic Steps

### 1. Gather Environment Info
Run these commands to understand the setup:
- `mcplug --version` — confirm binary version
- `mcplug config show` — display merged config with source annotations
- `echo $MCPLUG_CONFIG` — check for config path override
- `echo $MCPLUG_LOG_LEVEL` — check log verbosity

### 2. Check Config Files
Look for config files and validate them:
- `./config/mcplug.json` (project-level)
- `~/.mcplug/mcplug.json` (home-level)
- Check for JSONC syntax errors (mcplug strips `//` and `/* */` comments, but not trailing commas)
- Verify `mcpServers` key exists (not `mcp_servers` — JSON uses camelCase)

### 3. Diagnose by Error Type

#### Connection Errors (`connection_refused`)
- For stdio servers: verify `command` exists and is executable; check `args` are correct
- For HTTP servers: verify `baseUrl` is reachable; try `curl <baseUrl>` to test connectivity
- Check if env vars in config are set: `${VAR}` errors if `VAR` is unset
- Run with debug logging: `MCPLUG_LOG_LEVEL=debug mcplug list <server>`

#### Authentication Errors (`auth_required`)
- Run `mcplug auth <server>` to complete OAuth flow
- Check token cache: `~/.mcplug/<server>/tokens.json`
- Verify the server's `baseUrl` supports OAuth discovery at `.well-known/oauth-authorization-server`
- Check `MCPLUG_OAUTH_TIMEOUT_MS` if the browser flow is timing out (default: 60000ms)

#### Config Errors (`config_error`)
- Invalid JSON: run config through `python3 -m json.tool <config-file>` to find syntax errors
- Missing env vars: check all `${VAR}` references have corresponding env vars set
- Wrong precedence: remember earlier sources win — `--config` flag overrides everything

#### Timeout Errors (`timeout`)
- Default timeout is 30s for list/call operations
- For slow servers, check if the server process is starting correctly
- For HTTP servers, check network latency and server health

#### Daemon Issues
- `mcplug daemon status` — check if daemon is running
- Daemon only works with `"lifecycle": "keep-alive"` servers
- Daemon is **stdio-only** on Unix (uses PID files in `/tmp`)
- On Windows, daemon `stop()` prints "not supported"
- Check PID file: the daemon manager uses `/tmp` for state

### 4. Common Gotchas Checklist

- **Cleartext HTTP**: MCP servers should use HTTPS; cleartext `http://` URLs may cause issues with OAuth discovery
- **Env var timing**: Environment variables are expanded at config load time, not at connection time. If you change an env var, reload the config
- **Precedence direction**: Earlier sources **win** (not later). CLI flag beats env var beats project config beats home config
- **Stdio working directory**: Stdio child processes inherit the working directory of the mcplug process, not the directory of the config file
- **Daemon is stdio-only**: The daemon manages stdio child processes. HTTP/SSE connections don't need a daemon (they're stateless requests)
- **Token permissions**: OAuth token cache files at `~/.mcplug/<server>/tokens.json` need read/write permissions
- **JSONC vs JSON**: mcplug supports `//` and `/* */` comments in config files, but not trailing commas
- **camelCase in config**: Use `mcpServers`, `baseUrl` (not `mcp_servers`, `base_url`) — the JSON format uses camelCase
- **Both baseUrl and command**: If a server has both, `baseUrl` takes priority (HTTP transport is preferred)
- **Import deduplication**: If the same editor appears in multiple config files' `imports` arrays, it's only imported once

### 5. Verbose Debugging
If the above doesn't resolve the issue, suggest:
```bash
MCPLUG_LOG_LEVEL=trace mcplug list <server> 2>debug.log
```
Then read `debug.log` for detailed transport-level messages.
