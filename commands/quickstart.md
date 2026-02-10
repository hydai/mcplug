---
name: quickstart
description: Interactive quickstart guide — checks installation, discovers configs, and walks through first mcplug commands
allowed-tools:
  - Read
  - Glob
  - Grep
  - Bash
---

You are guiding a user through their first experience with mcplug. Be adaptive — read their actual environment rather than giving generic instructions.

## Steps

### 1. Check Installation
Run `mcplug --version` to verify the binary is available. If not found, suggest:
- `cargo install mcplug` (from crates.io)
- Or `cargo build --release` if they're in the mcplug repo

### 2. Discover Existing Configuration
Check for existing config files in order:
- `./config/mcplug.json` (project-level)
- `~/.mcplug/mcplug.json` or `~/.mcplug/mcplug.jsonc` (home-level)
- `~/.mcporter/mcporter.json` (mcporter fallback)

Report what you find: how many servers are configured, their names, and transport types.

### 3. Detect Editor Configs
Check for MCP configs from editors that could be imported:
- `~/.cursor/mcp.json`
- `~/.claude/.mcp.json`
- `~/.vscode/mcp.json`
- `~/.windsurf/mcp.json`
- `~/.codex/mcp.json`
- `~/.opencode/mcp.json`

If found, explain how to add `"imports": ["cursor"]` (etc.) to the mcplug config to reuse those servers.

### 4. Guide Config Creation (if needed)
If no config exists, help create `~/.mcplug/mcplug.json` with a minimal example:

```json
{
  "mcpServers": {
    "example": {
      "command": "npx",
      "args": ["-y", "some-mcp-server"]
    }
  }
}
```

Ask the user what MCP servers they want to use and tailor the config accordingly.

### 5. First Commands
Walk through:
1. `mcplug list` — show all configured servers and their tools
2. `mcplug list <server>` — show tools for a specific server
3. `mcplug call <server.tool> key:value` — make the first tool call

Use a server they actually have configured, not a hypothetical one.

### 6. Suggest Next Steps
Based on what they have configured, suggest:
- Using `--json` for scripting
- Setting up `"lifecycle": "keep-alive"` + `mcplug daemon start` for frequently-used servers
- Using `mcplug auth` if they have HTTP servers that might need OAuth
- Using mcplug as a Rust library with `call_once()` for programmatic access
- Importing editor configs if they have editors with MCP configured
