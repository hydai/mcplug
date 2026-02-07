use std::collections::HashMap;

use crate::config::McplugConfig;
use crate::error::McplugError;
use crate::transport::McpTransport;
use crate::transports::{HttpSseTransport, StdioTransport};

/// Create a transport connection to an MCP server.
///
/// Priority:
/// 1. If `http_url` is provided, create an HTTP transport
/// 2. If `stdio_cmd` is provided, parse and create a stdio transport
/// 3. Look up server in config and create the appropriate transport
pub fn connect_to_server(
    server_name: &str,
    config: &McplugConfig,
    http_url: Option<&str>,
    stdio_cmd: Option<&str>,
) -> Result<Box<dyn McpTransport>, McplugError> {
    // Ad-hoc HTTP URL
    if let Some(url) = http_url {
        let allow_http = url.starts_with("http://");
        let transport = HttpSseTransport::new(url, &HashMap::new(), server_name, allow_http)?;
        return Ok(Box::new(transport));
    }

    // Ad-hoc stdio command
    if let Some(cmd_str) = stdio_cmd {
        let parts: Vec<&str> = cmd_str.split_whitespace().collect();
        if parts.is_empty() {
            return Err(McplugError::ProtocolError(
                "Empty stdio command".to_string(),
            ));
        }
        let command = parts[0];
        let args: Vec<String> = parts[1..].iter().map(|s| s.to_string()).collect();
        let transport =
            StdioTransport::new(command, &args, &HashMap::new(), None, server_name)?;
        return Ok(Box::new(transport));
    }

    // Look up in config
    let server_config = config
        .mcp_servers
        .get(server_name)
        .ok_or_else(|| McplugError::ServerNotFound(server_name.to_string()))?;

    if let Some(ref base_url) = server_config.base_url {
        let transport = HttpSseTransport::new(
            base_url,
            &server_config.headers,
            server_name,
            false,
        )?;
        Ok(Box::new(transport))
    } else if let Some(ref command) = server_config.command {
        let transport = StdioTransport::new(
            command,
            &server_config.args,
            &server_config.env,
            None,
            server_name,
        )?;
        Ok(Box::new(transport))
    } else {
        Err(McplugError::ConnectionFailed {
            server: server_name.to_string(),
            source: "Server config has neither baseUrl nor command".into(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn connect_adhoc_stdio_echo() {
        let config = McplugConfig::default();
        let result = connect_to_server("test", &config, None, Some("cat"));
        assert!(result.is_ok());
    }

    #[test]
    fn connect_adhoc_stdio_empty_cmd() {
        let config = McplugConfig::default();
        let result = connect_to_server("test", &config, None, Some(""));
        assert!(result.is_err());
    }

    #[test]
    fn connect_adhoc_http_https() {
        let config = McplugConfig::default();
        let result = connect_to_server("test", &config, Some("https://example.com/mcp"), None);
        assert!(result.is_ok());
    }

    #[test]
    fn connect_adhoc_http_cleartext() {
        let config = McplugConfig::default();
        let result = connect_to_server("test", &config, Some("http://localhost:8080/mcp"), None);
        // allow_http is set automatically for http:// URLs in ad-hoc mode
        assert!(result.is_ok());
    }

    #[test]
    fn connect_server_not_found() {
        let config = McplugConfig::default();
        let result = connect_to_server("nonexistent", &config, None, None);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(err, McplugError::ServerNotFound(_)));
    }

    #[test]
    fn connect_config_http_server() {
        use crate::config::ServerConfig;
        let mut config = McplugConfig::default();
        config.mcp_servers.insert(
            "web".to_string(),
            ServerConfig {
                description: None,
                base_url: Some("https://example.com/mcp".into()),
                command: None,
                args: vec![],
                env: HashMap::new(),
                headers: HashMap::new(),
                lifecycle: None,
            },
        );
        let result = connect_to_server("web", &config, None, None);
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn connect_config_stdio_server() {
        use crate::config::ServerConfig;
        let mut config = McplugConfig::default();
        config.mcp_servers.insert(
            "local".to_string(),
            ServerConfig {
                description: None,
                base_url: None,
                command: Some("cat".into()),
                args: vec![],
                env: HashMap::new(),
                headers: HashMap::new(),
                lifecycle: None,
            },
        );
        let result = connect_to_server("local", &config, None, None);
        assert!(result.is_ok());
    }

    #[test]
    fn connect_config_no_transport() {
        use crate::config::ServerConfig;
        let mut config = McplugConfig::default();
        config.mcp_servers.insert(
            "empty".to_string(),
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
        let result = connect_to_server("empty", &config, None, None);
        assert!(result.is_err());
    }
}
