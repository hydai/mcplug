use async_trait::async_trait;

use crate::error::McplugError;
use crate::types::{CallResult, ServerInfo, ToolDefinition};

impl std::fmt::Debug for dyn McpTransport {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("McpTransport").finish()
    }
}

#[async_trait]
pub trait McpTransport: Send + Sync {
    /// Perform the MCP initialization handshake and return server info.
    async fn initialize(&mut self) -> Result<ServerInfo, McplugError>;

    /// List all tools available on this server.
    async fn list_tools(&self) -> Result<Vec<ToolDefinition>, McplugError>;

    /// Call a tool by name with the given arguments.
    async fn call_tool(&self, name: &str, args: serde_json::Value)
        -> Result<CallResult, McplugError>;

    /// Close the transport connection and clean up resources.
    async fn close(&mut self) -> Result<(), McplugError>;
}
