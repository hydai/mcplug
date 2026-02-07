pub mod args;
pub mod cli;
pub mod codegen;
pub mod config;
pub mod daemon;
pub mod error;
pub mod oauth;
pub mod runtime;
pub mod server_proxy;
pub mod transport;
pub mod transports;
pub mod types;

pub use config::{load_config, McplugConfig, ServerConfig};
pub use error::McplugError;
pub use runtime::Runtime;
pub use server_proxy::ServerProxy;
pub use transport::McpTransport;
pub use transports::{HttpSseTransport, StdioTransport};
pub use types::{CallResult, ContentBlock, ServerInfo, ToolDefinition};

/// One-shot convenience function: connect, call, disconnect.
pub async fn call_once(
    server: &str,
    tool: &str,
    args: serde_json::Value,
) -> Result<CallResult, McplugError> {
    let runtime = Runtime::from_config().await?;
    let result = runtime.call_tool(server, tool, args).await?;
    runtime.close().await?;
    Ok(result)
}
