use std::collections::HashMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McplugConfig {
    #[serde(default, rename = "mcpServers")]
    pub mcp_servers: HashMap<String, ServerConfig>,
    #[serde(default)]
    pub imports: Vec<String>,
}

impl Default for McplugConfig {
    fn default() -> Self {
        Self {
            mcp_servers: HashMap::new(),
            imports: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default, rename = "baseUrl")]
    pub base_url: Option<String>,
    #[serde(default)]
    pub command: Option<String>,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub env: HashMap<String, String>,
    #[serde(default)]
    pub headers: HashMap<String, String>,
    #[serde(default)]
    pub lifecycle: Option<Lifecycle>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Lifecycle {
    KeepAlive,
    Ephemeral,
}

#[derive(Debug, Clone)]
pub struct AnnotatedServerConfig {
    pub config: ServerConfig,
    pub source: PathBuf,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deserialize_http_server() {
        let json = r#"{
            "description": "A web scraper",
            "baseUrl": "https://mcp.example.com/mcp",
            "headers": {"Authorization": "Bearer tok"},
            "lifecycle": "keep-alive"
        }"#;
        let cfg: ServerConfig = serde_json::from_str(json).unwrap();
        assert_eq!(cfg.description.as_deref(), Some("A web scraper"));
        assert_eq!(cfg.base_url.as_deref(), Some("https://mcp.example.com/mcp"));
        assert!(matches!(cfg.lifecycle, Some(Lifecycle::KeepAlive)));
        assert_eq!(cfg.headers.get("Authorization").unwrap(), "Bearer tok");
    }

    #[test]
    fn deserialize_stdio_server() {
        let json = r#"{
            "command": "npx",
            "args": ["-y", "some-server"],
            "env": {"API_KEY": "secret"},
            "lifecycle": "ephemeral"
        }"#;
        let cfg: ServerConfig = serde_json::from_str(json).unwrap();
        assert_eq!(cfg.command.as_deref(), Some("npx"));
        assert_eq!(cfg.args, vec!["-y", "some-server"]);
        assert!(matches!(cfg.lifecycle, Some(Lifecycle::Ephemeral)));
    }

    #[test]
    fn deserialize_minimal_server() {
        let json = r#"{}"#;
        let cfg: ServerConfig = serde_json::from_str(json).unwrap();
        assert!(cfg.description.is_none());
        assert!(cfg.base_url.is_none());
        assert!(cfg.command.is_none());
        assert!(cfg.args.is_empty());
        assert!(cfg.env.is_empty());
        assert!(cfg.headers.is_empty());
        assert!(cfg.lifecycle.is_none());
    }

    #[test]
    fn deserialize_full_config() {
        let json = r#"{
            "mcpServers": {
                "firecrawl": {
                    "baseUrl": "https://mcp.firecrawl.dev/mcp"
                }
            },
            "imports": ["cursor", "claude-code"]
        }"#;
        let cfg: McplugConfig = serde_json::from_str(json).unwrap();
        assert!(cfg.mcp_servers.contains_key("firecrawl"));
        assert_eq!(cfg.imports, vec!["cursor", "claude-code"]);
    }

    #[test]
    fn serialize_roundtrip() {
        let mut servers = HashMap::new();
        servers.insert(
            "test".to_string(),
            ServerConfig {
                description: Some("test server".into()),
                base_url: None,
                command: Some("test-cmd".into()),
                args: vec!["--flag".into()],
                env: HashMap::new(),
                headers: HashMap::new(),
                lifecycle: Some(Lifecycle::Ephemeral),
            },
        );
        let cfg = McplugConfig {
            mcp_servers: servers,
            imports: vec!["cursor".into()],
        };
        let json = serde_json::to_string(&cfg).unwrap();
        let parsed: McplugConfig = serde_json::from_str(&json).unwrap();
        assert!(parsed.mcp_servers.contains_key("test"));
        assert_eq!(parsed.imports, vec!["cursor"]);
    }

    #[test]
    fn deserialize_config_with_unknown_fields_is_lenient() {
        let json = r#"{
            "mcpServers": {
                "s1": {
                    "command": "echo",
                    "unknownField": "ignored",
                    "anotherUnknown": 42
                }
            },
            "imports": [],
            "topLevelUnknown": true
        }"#;
        // serde(default) + deny_unknown_fields is NOT used, so unknown fields
        // should be silently skipped
        let cfg: McplugConfig = serde_json::from_str(json).unwrap();
        assert!(cfg.mcp_servers.contains_key("s1"));
        assert_eq!(
            cfg.mcp_servers.get("s1").unwrap().command.as_deref(),
            Some("echo")
        );
    }
}
