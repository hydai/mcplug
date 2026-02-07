use std::collections::HashMap;
use std::env;

use tokio::sync::Mutex;

use crate::config::types::{Lifecycle, McplugConfig, ServerConfig};
use crate::config::load_config;
use crate::error::McplugError;
use crate::transport::McpTransport;
use crate::transports::{HttpSseTransport, StdioTransport};
use crate::types::{CallResult, ServerInfo, ToolDefinition};

/// Manages connections to MCP servers based on the merged configuration.
pub struct Runtime {
    config: McplugConfig,
    connections: Mutex<HashMap<String, Box<dyn McpTransport>>>,
}

impl Runtime {
    /// Create a Runtime by loading and merging all config sources.
    pub async fn from_config() -> Result<Self, McplugError> {
        let config = load_config(None)?;
        Ok(Self {
            config,
            connections: Mutex::new(HashMap::new()),
        })
    }

    /// Create a Runtime from an existing config.
    pub fn with_config(config: McplugConfig) -> Self {
        Self {
            config,
            connections: Mutex::new(HashMap::new()),
        }
    }

    /// Call a tool on a given server, lazily connecting if needed.
    pub async fn call_tool(
        &self,
        server: &str,
        tool: &str,
        args: serde_json::Value,
    ) -> Result<CallResult, McplugError> {
        let mut conns = self.connections.lock().await;
        if !conns.contains_key(server) {
            let mut transport = self.create_transport(server)?;
            transport.initialize().await?;
            conns.insert(server.to_string(), transport);
        }
        conns
            .get(server)
            .unwrap()
            .call_tool(tool, args)
            .await
    }

    /// List tools available on a given server, lazily connecting if needed.
    pub async fn list_tools(&self, server: &str) -> Result<Vec<ToolDefinition>, McplugError> {
        let mut conns = self.connections.lock().await;
        if !conns.contains_key(server) {
            let mut transport = self.create_transport(server)?;
            transport.initialize().await?;
            conns.insert(server.to_string(), transport);
        }
        conns.get(server).unwrap().list_tools().await
    }

    /// Return server info by initializing (or reusing) a connection.
    pub async fn server_info(&self, server: &str) -> Result<ServerInfo, McplugError> {
        let mut conns = self.connections.lock().await;
        if !conns.contains_key(server) {
            let mut transport = self.create_transport(server)?;
            let info = transport.initialize().await?;
            conns.insert(server.to_string(), transport);
            return Ok(info);
        }
        // Already connected — re-list isn't ideal, but we don't cache ServerInfo.
        // For now, return a placeholder. A real impl might cache this.
        Ok(ServerInfo {
            name: server.to_string(),
            version: "unknown".to_string(),
            capabilities: serde_json::json!({}),
        })
    }

    /// Close all active connections.
    pub async fn close(&self) -> Result<(), McplugError> {
        let mut conns = self.connections.lock().await;
        for (_name, transport) in conns.iter_mut() {
            // close() requires &mut self on the trait, but we have Box<dyn McpTransport>.
            // We need to use the DerefMut on Box.
            transport.close().await?;
        }
        conns.clear();
        Ok(())
    }

    /// Return a reference to the loaded configuration.
    pub fn config(&self) -> &McplugConfig {
        &self.config
    }

    /// Return the list of configured server names.
    pub fn server_names(&self) -> Vec<String> {
        self.config.mcp_servers.keys().cloned().collect()
    }

    /// Resolve the effective lifecycle for a server, considering env overrides.
    #[allow(dead_code)]
    fn effective_lifecycle(&self, server: &str, cfg: &ServerConfig) -> Option<Lifecycle> {
        // MCPLUG_KEEPALIVE=server_name forces keep-alive
        if let Ok(val) = env::var("MCPLUG_KEEPALIVE") {
            if val == server || val == "*" {
                return Some(Lifecycle::KeepAlive);
            }
        }
        // MCPLUG_DISABLE_KEEPALIVE=server_name forces ephemeral
        if let Ok(val) = env::var("MCPLUG_DISABLE_KEEPALIVE") {
            if val == server || val == "*" {
                return Some(Lifecycle::Ephemeral);
            }
        }
        cfg.lifecycle.clone()
    }

    /// Create a transport for the given server name based on its config.
    fn create_transport(
        &self,
        server: &str,
    ) -> Result<Box<dyn McpTransport>, McplugError> {
        let cfg = self
            .config
            .mcp_servers
            .get(server)
            .ok_or_else(|| McplugError::ServerNotFound(server.to_string()))?;

        if let Some(ref base_url) = cfg.base_url {
            let transport = HttpSseTransport::new(
                base_url,
                &cfg.headers,
                server,
                false,
            )?;
            Ok(Box::new(transport))
        } else if let Some(ref command) = cfg.command {
            let transport = StdioTransport::new(
                command,
                &cfg.args,
                &cfg.env,
                None,
                server,
            )?;
            Ok(Box::new(transport))
        } else {
            Err(McplugError::ConfigError {
                path: std::path::PathBuf::from("<runtime>"),
                detail: format!(
                    "Server '{}' has neither 'baseUrl' nor 'command' configured",
                    server
                ),
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::types::ServerConfig;
    use std::sync::Mutex;

    /// Mutex to serialize tests that manipulate MCPLUG_KEEPALIVE / MCPLUG_DISABLE_KEEPALIVE
    /// env vars, preventing race conditions in parallel test execution.
    static LIFECYCLE_ENV_LOCK: Mutex<()> = Mutex::new(());

    fn make_stdio_config() -> McplugConfig {
        let mut servers = HashMap::new();
        servers.insert(
            "echo".to_string(),
            ServerConfig {
                description: Some("Echo server".into()),
                base_url: None,
                command: Some("cat".into()),
                args: vec![],
                env: HashMap::new(),
                headers: HashMap::new(),
                lifecycle: None,
            },
        );
        servers.insert(
            "http-server".to_string(),
            ServerConfig {
                description: Some("HTTP server".into()),
                base_url: Some("https://example.com/mcp".into()),
                command: None,
                args: vec![],
                env: HashMap::new(),
                headers: HashMap::new(),
                lifecycle: Some(Lifecycle::KeepAlive),
            },
        );
        McplugConfig {
            mcp_servers: servers,
            imports: vec![],
        }
    }

    #[test]
    fn runtime_with_config_has_servers() {
        let config = make_stdio_config();
        let runtime = Runtime::with_config(config);
        let names = runtime.server_names();
        assert!(names.contains(&"echo".to_string()));
        assert!(names.contains(&"http-server".to_string()));
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn create_transport_stdio() {
        let config = make_stdio_config();
        let runtime = Runtime::with_config(config);
        let transport = runtime.create_transport("echo");
        assert!(transport.is_ok());
    }

    #[test]
    fn create_transport_http() {
        let config = make_stdio_config();
        let runtime = Runtime::with_config(config);
        let transport = runtime.create_transport("http-server");
        assert!(transport.is_ok());
    }

    #[test]
    fn create_transport_not_found() {
        let config = make_stdio_config();
        let runtime = Runtime::with_config(config);
        let err = runtime.create_transport("nonexistent").unwrap_err();
        assert!(matches!(err, McplugError::ServerNotFound(_)));
    }

    #[test]
    fn create_transport_no_command_or_url() {
        let mut servers = HashMap::new();
        servers.insert(
            "broken".to_string(),
            ServerConfig {
                description: None,
                base_url: None,
                command: None,
                args: vec![],
                env: HashMap::new(),
                headers: HashMap::new(),
                lifecycle: None,
            },
        );
        let config = McplugConfig {
            mcp_servers: servers,
            imports: vec![],
        };
        let runtime = Runtime::with_config(config);
        let err = runtime.create_transport("broken").unwrap_err();
        assert!(err.to_string().contains("neither"));
    }

    #[test]
    fn config_accessor() {
        let config = make_stdio_config();
        let runtime = Runtime::with_config(config);
        assert!(runtime.config().mcp_servers.contains_key("echo"));
    }

    #[test]
    fn effective_lifecycle_default() {
        let _lock = LIFECYCLE_ENV_LOCK.lock().unwrap();
        env::remove_var("MCPLUG_KEEPALIVE");
        env::remove_var("MCPLUG_DISABLE_KEEPALIVE");
        let config = make_stdio_config();
        let runtime = Runtime::with_config(config);
        let cfg = runtime.config.mcp_servers.get("echo").unwrap();
        let lc = runtime.effective_lifecycle("echo", cfg);
        assert!(lc.is_none());
    }

    #[test]
    fn effective_lifecycle_from_config() {
        let _lock = LIFECYCLE_ENV_LOCK.lock().unwrap();
        env::remove_var("MCPLUG_KEEPALIVE");
        env::remove_var("MCPLUG_DISABLE_KEEPALIVE");
        let config = make_stdio_config();
        let runtime = Runtime::with_config(config);
        let cfg = runtime.config.mcp_servers.get("http-server").unwrap();
        let lc = runtime.effective_lifecycle("http-server", cfg);
        assert!(matches!(lc, Some(Lifecycle::KeepAlive)));
    }

    #[test]
    fn server_names_returns_all_configured() {
        let config = make_stdio_config();
        let runtime = Runtime::with_config(config);
        let mut names = runtime.server_names();
        names.sort();
        assert_eq!(names, vec!["echo", "http-server"]);
    }

    #[test]
    fn effective_lifecycle_env_keepalive_override() {
        let _lock = LIFECYCLE_ENV_LOCK.lock().unwrap();
        env::remove_var("MCPLUG_KEEPALIVE");
        env::remove_var("MCPLUG_DISABLE_KEEPALIVE");

        let config = make_stdio_config();
        let runtime = Runtime::with_config(config);
        let cfg = runtime.config.mcp_servers.get("echo").unwrap();

        env::set_var("MCPLUG_KEEPALIVE", "echo");
        let lc = runtime.effective_lifecycle("echo", cfg);
        env::remove_var("MCPLUG_KEEPALIVE");

        assert!(matches!(lc, Some(Lifecycle::KeepAlive)));
    }

    #[test]
    fn effective_lifecycle_env_disable_keepalive_override() {
        let _lock = LIFECYCLE_ENV_LOCK.lock().unwrap();
        env::remove_var("MCPLUG_KEEPALIVE");
        env::remove_var("MCPLUG_DISABLE_KEEPALIVE");

        let config = make_stdio_config();
        let runtime = Runtime::with_config(config);
        let cfg = runtime.config.mcp_servers.get("http-server").unwrap();

        env::set_var("MCPLUG_DISABLE_KEEPALIVE", "http-server");
        let lc = runtime.effective_lifecycle("http-server", cfg);
        env::remove_var("MCPLUG_DISABLE_KEEPALIVE");

        assert!(matches!(lc, Some(Lifecycle::Ephemeral)));
    }

    #[test]
    fn effective_lifecycle_wildcard_keepalive() {
        let _lock = LIFECYCLE_ENV_LOCK.lock().unwrap();
        env::remove_var("MCPLUG_KEEPALIVE");
        env::remove_var("MCPLUG_DISABLE_KEEPALIVE");

        let config = make_stdio_config();
        let runtime = Runtime::with_config(config);
        let cfg = runtime.config.mcp_servers.get("echo").unwrap();

        env::set_var("MCPLUG_KEEPALIVE", "*");
        let lc = runtime.effective_lifecycle("echo", cfg);
        env::remove_var("MCPLUG_KEEPALIVE");

        assert!(matches!(lc, Some(Lifecycle::KeepAlive)));
    }

    #[tokio::test]
    async fn close_empty_runtime_succeeds() {
        let config = McplugConfig {
            mcp_servers: HashMap::new(),
            imports: vec![],
        };
        let runtime = Runtime::with_config(config);
        // Closing a runtime with no active connections should succeed
        let result = runtime.close().await;
        assert!(result.is_ok());
    }

    #[test]
    fn create_transport_prefers_base_url_over_command() {
        // A server with both baseUrl and command should prefer baseUrl (HTTP transport)
        let mut servers = HashMap::new();
        servers.insert(
            "both".to_string(),
            ServerConfig {
                description: None,
                base_url: Some("https://example.com/mcp".into()),
                command: Some("echo".into()),
                args: vec![],
                env: HashMap::new(),
                headers: HashMap::new(),
                lifecycle: None,
            },
        );
        let config = McplugConfig {
            mcp_servers: servers,
            imports: vec![],
        };
        let runtime = Runtime::with_config(config);
        let transport = runtime.create_transport("both");
        // Should succeed — create_transport checks base_url first
        assert!(transport.is_ok());
    }
}
