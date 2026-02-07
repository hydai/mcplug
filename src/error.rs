use std::path::PathBuf;
use std::time::Duration;

#[derive(Debug, thiserror::Error)]
pub enum McplugError {
    #[error("Server '{0}' not found. Available: (none loaded)")]
    ServerNotFound(String),

    #[error("Tool '{tool}' not found on {server}.")]
    ToolNotFound { server: String, tool: String },

    #[error("Cannot connect to {server}: {source}")]
    ConnectionFailed {
        server: String,
        source: Box<dyn std::error::Error + Send + Sync>,
    },

    #[error("{}", format_timeout(.server, .tool.as_deref(), .duration))]
    Timeout {
        server: String,
        tool: Option<String>,
        duration: Duration,
    },

    #[error("Server '{0}' requires authentication. Run: mcplug auth {0}")]
    AuthRequired(String),

    #[error("Error in config {}: {detail}", path.display())]
    ConfigError { path: PathBuf, detail: String },

    #[error("Transport error: {0}")]
    TransportError(Box<dyn std::error::Error + Send + Sync>),

    #[error("Protocol error: {0}")]
    ProtocolError(String),

    #[error("OAuth error: {0}")]
    OAuthError(String),

    #[error("I/O error: {0}")]
    IoError(#[from] std::io::Error),
}

fn format_timeout(server: &str, tool: Option<&str>, duration: &Duration) -> String {
    let secs = duration.as_secs();
    match tool {
        Some(t) => format!("Timeout after {secs}s calling {server}.{t}"),
        None => format!("Timeout after {secs}s calling {server}"),
    }
}

impl McplugError {
    /// Error code string for structured JSON output.
    pub fn code(&self) -> &'static str {
        match self {
            McplugError::ServerNotFound(_) => "not_found",
            McplugError::ToolNotFound { .. } => "not_found",
            McplugError::ConnectionFailed { .. } => "connection_refused",
            McplugError::Timeout { .. } => "timeout",
            McplugError::AuthRequired(_) => "auth_required",
            McplugError::ConfigError { .. } => "config_error",
            McplugError::TransportError(_) => "transport_error",
            McplugError::ProtocolError(_) => "parse_error",
            McplugError::OAuthError(_) => "oauth_error",
            McplugError::IoError(_) => "io_error",
        }
    }

    pub fn server_name(&self) -> Option<&str> {
        match self {
            McplugError::ServerNotFound(s) => Some(s),
            McplugError::ToolNotFound { server, .. } => Some(server),
            McplugError::ConnectionFailed { server, .. } => Some(server),
            McplugError::Timeout { server, .. } => Some(server),
            McplugError::AuthRequired(s) => Some(s),
            _ => None,
        }
    }

    pub fn tool_name(&self) -> Option<&str> {
        match self {
            McplugError::ToolNotFound { tool, .. } => Some(tool),
            McplugError::Timeout { tool, .. } => tool.as_deref(),
            _ => None,
        }
    }

    /// Produce a structured JSON error object per SPEC.
    pub fn to_json(&self) -> serde_json::Value {
        let mut obj = serde_json::Map::new();
        if let Some(server) = self.server_name() {
            obj.insert("server".into(), serde_json::Value::String(server.to_string()));
        }
        if let Some(tool) = self.tool_name() {
            obj.insert("tool".into(), serde_json::Value::String(tool.to_string()));
        }
        obj.insert("message".into(), serde_json::Value::String(self.to_string()));
        obj.insert("code".into(), serde_json::Value::String(self.code().to_string()));
        serde_json::json!({ "error": obj })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_server_not_found() {
        let err = McplugError::ServerNotFound("myserver".into());
        assert_eq!(
            err.to_string(),
            "Server 'myserver' not found. Available: (none loaded)"
        );
    }

    #[test]
    fn display_tool_not_found() {
        let err = McplugError::ToolNotFound {
            server: "firecrawl".into(),
            tool: "scrap".into(),
        };
        assert_eq!(err.to_string(), "Tool 'scrap' not found on firecrawl.");
    }

    #[test]
    fn display_connection_failed() {
        let err = McplugError::ConnectionFailed {
            server: "myserver".into(),
            source: "connection refused".into(),
        };
        assert_eq!(
            err.to_string(),
            "Cannot connect to myserver: connection refused"
        );
    }

    #[test]
    fn display_timeout_with_tool() {
        let err = McplugError::Timeout {
            server: "firecrawl".into(),
            tool: Some("crawl".into()),
            duration: Duration::from_secs(30),
        };
        assert_eq!(
            err.to_string(),
            "Timeout after 30s calling firecrawl.crawl"
        );
    }

    #[test]
    fn display_timeout_without_tool() {
        let err = McplugError::Timeout {
            server: "firecrawl".into(),
            tool: None,
            duration: Duration::from_secs(30),
        };
        assert_eq!(err.to_string(), "Timeout after 30s calling firecrawl");
    }

    #[test]
    fn display_auth_required() {
        let err = McplugError::AuthRequired("github".into());
        assert_eq!(
            err.to_string(),
            "Server 'github' requires authentication. Run: mcplug auth github"
        );
    }

    #[test]
    fn display_config_error() {
        let err = McplugError::ConfigError {
            path: PathBuf::from("/home/user/.mcplug/mcplug.json"),
            detail: "invalid JSON".into(),
        };
        assert_eq!(
            err.to_string(),
            "Error in config /home/user/.mcplug/mcplug.json: invalid JSON"
        );
    }

    #[test]
    fn display_transport_error() {
        let err = McplugError::TransportError("pipe broken".into());
        assert_eq!(err.to_string(), "Transport error: pipe broken");
    }

    #[test]
    fn display_protocol_error() {
        let err = McplugError::ProtocolError("unexpected message type".into());
        assert_eq!(
            err.to_string(),
            "Protocol error: unexpected message type"
        );
    }

    // --- P2/P3 additional tests ---

    #[test]
    fn error_code_mapping_all_variants() {
        assert_eq!(McplugError::ServerNotFound("s".into()).code(), "not_found");
        assert_eq!(
            McplugError::ToolNotFound {
                server: "s".into(),
                tool: "t".into()
            }
            .code(),
            "not_found"
        );
        assert_eq!(
            McplugError::ConnectionFailed {
                server: "s".into(),
                source: "err".into()
            }
            .code(),
            "connection_refused"
        );
        assert_eq!(
            McplugError::Timeout {
                server: "s".into(),
                tool: None,
                duration: Duration::from_secs(1)
            }
            .code(),
            "timeout"
        );
        assert_eq!(McplugError::AuthRequired("s".into()).code(), "auth_required");
        assert_eq!(
            McplugError::ConfigError {
                path: PathBuf::from("/a"),
                detail: "d".into()
            }
            .code(),
            "config_error"
        );
        assert_eq!(
            McplugError::TransportError("e".into()).code(),
            "transport_error"
        );
        assert_eq!(McplugError::ProtocolError("e".into()).code(), "parse_error");
        assert_eq!(McplugError::OAuthError("e".into()).code(), "oauth_error");
        let io_err = std::io::Error::new(std::io::ErrorKind::Other, "test");
        assert_eq!(McplugError::IoError(io_err).code(), "io_error");
    }

    #[test]
    fn error_to_json_structure() {
        let err = McplugError::ToolNotFound {
            server: "myserver".into(),
            tool: "mytool".into(),
        };
        let json = err.to_json();
        let error_obj = json.get("error").expect("should have error key");
        assert_eq!(error_obj["server"], "myserver");
        assert_eq!(error_obj["tool"], "mytool");
        assert_eq!(error_obj["code"], "not_found");
        assert!(error_obj["message"].as_str().unwrap().contains("mytool"));
    }

    #[test]
    fn error_server_name_accessors() {
        assert_eq!(
            McplugError::ServerNotFound("s1".into()).server_name(),
            Some("s1")
        );
        assert_eq!(
            McplugError::ToolNotFound {
                server: "s2".into(),
                tool: "t".into()
            }
            .server_name(),
            Some("s2")
        );
        assert_eq!(
            McplugError::ConnectionFailed {
                server: "s3".into(),
                source: "err".into()
            }
            .server_name(),
            Some("s3")
        );
        assert_eq!(
            McplugError::Timeout {
                server: "s4".into(),
                tool: None,
                duration: Duration::from_secs(1)
            }
            .server_name(),
            Some("s4")
        );
        assert_eq!(
            McplugError::AuthRequired("s5".into()).server_name(),
            Some("s5")
        );
        // Variants without server_name
        assert_eq!(McplugError::ProtocolError("e".into()).server_name(), None);
        assert_eq!(McplugError::TransportError("e".into()).server_name(), None);
        assert_eq!(McplugError::OAuthError("e".into()).server_name(), None);
    }

    #[test]
    fn error_tool_name_accessors() {
        assert_eq!(
            McplugError::ToolNotFound {
                server: "s".into(),
                tool: "mytool".into()
            }
            .tool_name(),
            Some("mytool")
        );
        assert_eq!(
            McplugError::Timeout {
                server: "s".into(),
                tool: Some("atool".into()),
                duration: Duration::from_secs(1)
            }
            .tool_name(),
            Some("atool")
        );
        assert_eq!(
            McplugError::Timeout {
                server: "s".into(),
                tool: None,
                duration: Duration::from_secs(1)
            }
            .tool_name(),
            None
        );
        assert_eq!(McplugError::ServerNotFound("s".into()).tool_name(), None);
        assert_eq!(McplugError::ProtocolError("e".into()).tool_name(), None);
    }
}
