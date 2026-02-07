use crate::error::McplugError;
use crate::runtime::Runtime;
use crate::types::CallResult;

/// A typed proxy for a specific MCP server, wrapping a Runtime.
pub struct ServerProxy<'a> {
    runtime: &'a Runtime,
    server: String,
}

impl<'a> ServerProxy<'a> {
    pub fn new(runtime: &'a Runtime, server: &str) -> Self {
        Self {
            runtime,
            server: server.to_string(),
        }
    }

    pub async fn call(
        &self,
        tool: &str,
        args: serde_json::Value,
    ) -> Result<CallResult, McplugError> {
        self.runtime.call_tool(&self.server, tool, args).await
    }

    pub fn server_name(&self) -> &str {
        &self.server
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::types::McplugConfig;
    use std::collections::HashMap;

    #[test]
    fn server_proxy_name() {
        let config = McplugConfig {
            mcp_servers: HashMap::new(),
            imports: vec![],
        };
        let runtime = Runtime::with_config(config);
        let proxy = ServerProxy::new(&runtime, "test-server");
        assert_eq!(proxy.server_name(), "test-server");
    }
}
