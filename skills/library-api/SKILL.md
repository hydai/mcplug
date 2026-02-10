---
description: |
  mcplug Rust library API — call_once, Runtime, ServerProxy, types, and errors.
  Use when the user asks about: mcplug library, mcplug crate, call_once, Runtime,
  ServerProxy, McpTransport, CallResult, McplugError, using mcplug as a dependency.
---

# mcplug Library API

mcplug is dual-purpose: CLI binary + library crate. Add it as a dependency:

```toml
[dependencies]
mcplug = "0.1"
```

## Public Re-exports (from `src/lib.rs`)

```rust
pub use config::{load_config, McplugConfig, ServerConfig};
pub use error::McplugError;
pub use runtime::Runtime;
pub use server_proxy::ServerProxy;
pub use transport::McpTransport;
pub use transports::{HttpSseTransport, StdioTransport};
pub use types::{CallResult, ContentBlock, ServerInfo, ToolDefinition};
```

## 3 Usage Patterns

### 1. One-shot: `call_once`

The simplest way — connect, call, disconnect in one function:

```rust
use mcplug::call_once;
use serde_json::json;

let result = call_once("firecrawl", "scrape", json!({"url": "https://example.com"})).await?;
println!("{}", result.text());
```

`call_once` loads config, creates a `Runtime`, calls the tool, and closes the connection. Source: `src/lib.rs`.

### 2. Connection Pooling: `Runtime`

For multiple calls, `Runtime` reuses connections (lazy connect on first call per server):

```rust
use mcplug::Runtime;
use serde_json::json;

let runtime = Runtime::from_config().await?;

// Connections are created lazily and reused
let tools = runtime.list_tools("firecrawl").await?;
let result = runtime.call_tool("firecrawl", "scrape", json!({"url": "https://example.com"})).await?;
let info = runtime.server_info("firecrawl").await?;

// Access config and server names
let config = runtime.config();
let names = runtime.server_names();

runtime.close().await?;
```

You can also create a Runtime from an existing config:

```rust
let runtime = Runtime::with_config(my_config);
```

Source: `src/runtime.rs`.

### 3. Typed Proxy: `ServerProxy`

Wraps a Runtime for a single server — avoids repeating the server name:

```rust
use mcplug::{Runtime, ServerProxy};
use serde_json::json;

let runtime = Runtime::from_config().await?;
let firecrawl = ServerProxy::new(&runtime, "firecrawl");

let result = firecrawl.call("scrape", json!({"url": "https://example.com"})).await?;
println!("Server: {}", firecrawl.server_name());
```

Source: `src/server_proxy.rs`.

## CallResult Methods

| Method | Return Type | Description |
|--------|-------------|-------------|
| `.text()` | `String` | Plain text from all Text and Resource blocks, joined by newlines |
| `.json::<T>()` | `Result<T, McplugError>` | Deserialize the text content as a typed value |
| `.markdown()` | `String` | Format all content blocks as markdown |
| `.content()` | `&[ContentBlock]` | Return the raw content blocks |
| `.raw()` | `Option<&serde_json::Value>` | Return the full raw MCP response envelope |

Source: `src/types.rs`.

## ContentBlock Enum

```rust
pub enum ContentBlock {
    Text { text: String },
    Image { data: String, mime_type: String },
    Resource { uri: String, text: String },
}
```

## McpTransport Trait

The core abstraction — both `StdioTransport` and `HttpSseTransport` implement this:

```rust
#[async_trait]
pub trait McpTransport: Send + Sync {
    async fn initialize(&mut self) -> Result<ServerInfo, McplugError>;
    async fn list_tools(&self) -> Result<Vec<ToolDefinition>, McplugError>;
    async fn call_tool(&self, name: &str, args: Value) -> Result<CallResult, McplugError>;
    async fn close(&mut self) -> Result<(), McplugError>;
}
```

Source: `src/transport.rs`.

## McplugError Enum (10 variants)

| Variant | Error Code | Description |
|---------|------------|-------------|
| `ServerNotFound(String)` | `not_found` | Server name not in config |
| `ToolNotFound { server, tool }` | `not_found` | Tool not found on server |
| `ConnectionFailed { server, source }` | `connection_refused` | Cannot connect to server |
| `Timeout { server, tool, duration }` | `timeout` | Operation timed out |
| `AuthRequired(String)` | `auth_required` | Server needs OAuth; run `mcplug auth` |
| `ConfigError { path, detail }` | `config_error` | Config file problem |
| `TransportError(Box<dyn Error>)` | `transport_error` | Transport-level failure |
| `ProtocolError(String)` | `parse_error` | JSON-RPC or argument parsing error |
| `OAuthError(String)` | `oauth_error` | OAuth flow failure |
| `IoError(io::Error)` | `io_error` | File system or I/O error |

Every variant has a `.code()` method returning the error code string, and `.to_json()` for structured JSON output.

Source: `src/error.rs`.

## Key Source Files

- `src/lib.rs` — Library root, public re-exports, `call_once`
- `src/runtime.rs` — `Runtime` struct, connection pooling, transport creation
- `src/server_proxy.rs` — `ServerProxy` typed wrapper
- `src/transport.rs` — `McpTransport` trait definition
- `src/types.rs` — `CallResult`, `ContentBlock`, `ServerInfo`, `ToolDefinition`
- `src/error.rs` — `McplugError` enum with 10 variants and error codes
