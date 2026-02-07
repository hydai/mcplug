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

    fn empty_runtime() -> Runtime {
        Runtime::with_config(McplugConfig {
            mcp_servers: HashMap::new(),
            imports: vec![],
        })
    }

    #[test]
    fn server_proxy_name() {
        let runtime = empty_runtime();
        let proxy = ServerProxy::new(&runtime, "test-server");
        assert_eq!(proxy.server_name(), "test-server");
    }

    #[test]
    fn server_proxy_creation_with_different_names() {
        let runtime = empty_runtime();
        let proxy_a = ServerProxy::new(&runtime, "server-alpha");
        let proxy_b = ServerProxy::new(&runtime, "server-beta");
        assert_eq!(proxy_a.server_name(), "server-alpha");
        assert_eq!(proxy_b.server_name(), "server-beta");
        assert_ne!(proxy_a.server_name(), proxy_b.server_name());
    }

    #[test]
    fn server_proxy_name_is_owned_copy() {
        let runtime = empty_runtime();
        let name = String::from("ephemeral-name");
        let proxy = ServerProxy::new(&runtime, &name);
        drop(name);
        // The proxy should still hold a valid owned copy
        assert_eq!(proxy.server_name(), "ephemeral-name");
    }

    #[tokio::test]
    async fn server_proxy_call_nonexistent_server_errors() {
        let runtime = empty_runtime();
        let proxy = ServerProxy::new(&runtime, "no-such-server");
        let result = proxy.call("some_tool", serde_json::json!({})).await;
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(err, McplugError::ServerNotFound(_)));
    }

    #[test]
    fn server_proxy_empty_server_name() {
        let runtime = empty_runtime();
        let proxy = ServerProxy::new(&runtime, "");
        assert_eq!(proxy.server_name(), "");
    }

    #[test]
    fn server_proxy_multiple_on_same_runtime() {
        let runtime = empty_runtime();
        let proxies: Vec<ServerProxy> = (0..5)
            .map(|i| ServerProxy::new(&runtime, &format!("server-{i}")))
            .collect();
        for (i, proxy) in proxies.iter().enumerate() {
            assert_eq!(proxy.server_name(), format!("server-{i}"));
        }
    }

    #[test]
    fn server_proxy_server_name_with_special_chars() {
        let runtime = empty_runtime();
        let proxy = ServerProxy::new(&runtime, "my-server/v2@prod");
        assert_eq!(proxy.server_name(), "my-server/v2@prod");
    }

    #[tokio::test]
    async fn server_proxy_call_with_empty_args() {
        let runtime = empty_runtime();
        let proxy = ServerProxy::new(&runtime, "nonexistent");
        let result = proxy.call("tool", serde_json::json!({})).await;
        // Should error because the server doesn't exist, not because of empty args
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), McplugError::ServerNotFound(_)));
    }
}
