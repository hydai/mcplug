use std::collections::HashMap;
use std::sync::Mutex;

use async_trait::async_trait;
use reqwest::header::{HeaderMap, HeaderName, HeaderValue};
use serde_json::json;

use crate::error::McplugError;
use crate::transport::McpTransport;
use crate::types::{CallResult, ServerInfo, ToolDefinition};

use super::jsonrpc::{JsonRpcNotification, JsonRpcResponse, RequestBuilder};

/// MCP HTTP Streamable transport.
///
/// Sends JSON-RPC requests as HTTP POST to a base URL and parses
/// the JSON-RPC response from the response body.
pub struct HttpSseTransport {
    client: reqwest::Client,
    base_url: String,
    server_name: String,
    session_id: Mutex<Option<String>>,
    // RequestBuilder doesn't derive Debug, so we implement Debug manually below
    request_builder: RequestBuilder,
}

impl std::fmt::Debug for HttpSseTransport {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HttpSseTransport")
            .field("base_url", &self.base_url)
            .field("server_name", &self.server_name)
            .field("session_id", &self.session_id)
            .finish_non_exhaustive()
    }
}

impl HttpSseTransport {
    /// Create a new HTTP transport.
    ///
    /// Rejects cleartext `http://` URLs unless `allow_http` is `true`.
    pub fn new(
        base_url: &str,
        headers: &HashMap<String, String>,
        server_name: &str,
        allow_http: bool,
    ) -> Result<Self, McplugError> {
        // Validate URL scheme
        let url = reqwest::Url::parse(base_url).map_err(|e| McplugError::ConnectionFailed {
            server: server_name.to_string(),
            source: format!("Invalid URL '{base_url}': {e}").into(),
        })?;

        match url.scheme() {
            "https" => {}
            "http" if allow_http => {}
            "http" => {
                return Err(McplugError::ConnectionFailed {
                    server: server_name.to_string(),
                    source: format!(
                        "Cleartext HTTP is not allowed for '{base_url}'. \
                         Use https:// or pass --allow-http to permit insecure connections."
                    )
                    .into(),
                });
            }
            scheme => {
                return Err(McplugError::ConnectionFailed {
                    server: server_name.to_string(),
                    source: format!("Unsupported URL scheme '{scheme}' in '{base_url}'").into(),
                });
            }
        }

        // Build default headers from the user-provided map
        let mut header_map = HeaderMap::new();
        header_map.insert(
            reqwest::header::CONTENT_TYPE,
            HeaderValue::from_static("application/json"),
        );
        header_map.insert(
            reqwest::header::ACCEPT,
            HeaderValue::from_static("application/json"),
        );

        for (key, value) in headers {
            let name = HeaderName::from_bytes(key.as_bytes()).map_err(|e| {
                McplugError::ConnectionFailed {
                    server: server_name.to_string(),
                    source: format!("Invalid header name '{key}': {e}").into(),
                }
            })?;
            let val =
                HeaderValue::from_str(value).map_err(|e| McplugError::ConnectionFailed {
                    server: server_name.to_string(),
                    source: format!("Invalid header value for '{key}': {e}").into(),
                })?;
            header_map.insert(name, val);
        }

        let client = reqwest::Client::builder()
            .default_headers(header_map)
            .build()
            .map_err(|e| McplugError::ConnectionFailed {
                server: server_name.to_string(),
                source: Box::new(e),
            })?;

        Ok(Self {
            client,
            base_url: base_url.to_string(),
            server_name: server_name.to_string(),
            session_id: Mutex::new(None),
            request_builder: RequestBuilder::new(),
        })
    }

    /// Send a JSON-RPC request and return the parsed response.
    async fn send_request(
        &self,
        method: &str,
        params: Option<serde_json::Value>,
    ) -> Result<serde_json::Value, McplugError> {
        let req = self.request_builder.next_request(method, params);

        let mut http_req = self.client.post(&self.base_url);

        // Attach session ID if we have one
        if let Ok(guard) = self.session_id.lock() {
            if let Some(ref sid) = *guard {
                http_req = http_req.header("Mcp-Session-Id", sid);
            }
        }

        let response = http_req.json(&req).send().await.map_err(|e| {
            McplugError::ConnectionFailed {
                server: self.server_name.clone(),
                source: Box::new(e),
            }
        })?;

        // Check HTTP status
        let status = response.status();
        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            return Err(McplugError::ConnectionFailed {
                server: self.server_name.clone(),
                source: format!("HTTP {status}: {body}").into(),
            });
        }

        // Extract session ID from response headers
        if let Some(sid) = response.headers().get("Mcp-Session-Id") {
            if let Ok(sid_str) = sid.to_str() {
                if let Ok(mut guard) = self.session_id.lock() {
                    *guard = Some(sid_str.to_string());
                }
            }
        }

        let rpc_response: JsonRpcResponse =
            response.json().await.map_err(|e| {
                McplugError::ProtocolError(format!(
                    "Failed to parse JSON-RPC response from {}: {e}",
                    self.server_name
                ))
            })?;

        // Check for JSON-RPC error
        if let Some(err) = rpc_response.error {
            return Err(McplugError::ProtocolError(format!(
                "JSON-RPC error {}: {}{}",
                err.code,
                err.message,
                err.data
                    .map(|d| format!(" ({})", d))
                    .unwrap_or_default()
            )));
        }

        rpc_response.result.ok_or_else(|| {
            McplugError::ProtocolError(
                "JSON-RPC response missing both 'result' and 'error'".to_string(),
            )
        })
    }

    /// Send a JSON-RPC notification (no response expected).
    async fn send_notification(
        &self,
        method: &str,
        params: Option<serde_json::Value>,
    ) -> Result<(), McplugError> {
        let notif = JsonRpcNotification {
            jsonrpc: "2.0".to_string(),
            method: method.to_string(),
            params,
        };

        let mut http_req = self.client.post(&self.base_url);

        if let Ok(guard) = self.session_id.lock() {
            if let Some(ref sid) = *guard {
                http_req = http_req.header("Mcp-Session-Id", sid);
            }
        }

        let response = http_req.json(&notif).send().await.map_err(|e| {
            McplugError::ConnectionFailed {
                server: self.server_name.clone(),
                source: Box::new(e),
            }
        })?;

        let status = response.status();
        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            return Err(McplugError::ConnectionFailed {
                server: self.server_name.clone(),
                source: format!("HTTP {status}: {body}").into(),
            });
        }

        Ok(())
    }
}

#[async_trait]
impl McpTransport for HttpSseTransport {
    async fn initialize(&mut self) -> Result<ServerInfo, McplugError> {
        let result = self
            .send_request(
                "initialize",
                Some(json!({
                    "protocolVersion": "2025-03-26",
                    "capabilities": {},
                    "clientInfo": {
                        "name": "mcplug",
                        "version": env!("CARGO_PKG_VERSION"),
                    }
                })),
            )
            .await?;

        // Send initialized notification
        self.send_notification("notifications/initialized", None)
            .await?;

        // Extract server info from response
        let server_info = result
            .get("serverInfo")
            .ok_or_else(|| {
                McplugError::ProtocolError("Initialize response missing 'serverInfo'".to_string())
            })?;

        let name = server_info
            .get("name")
            .and_then(|v| v.as_str())
            .unwrap_or(&self.server_name)
            .to_string();
        let version = server_info
            .get("version")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown")
            .to_string();
        let capabilities = result
            .get("capabilities")
            .cloned()
            .unwrap_or(json!({}));

        Ok(ServerInfo {
            name,
            version,
            capabilities,
        })
    }

    async fn list_tools(&self) -> Result<Vec<ToolDefinition>, McplugError> {
        let result = self.send_request("tools/list", None).await?;

        let tools_value = result.get("tools").ok_or_else(|| {
            McplugError::ProtocolError("tools/list response missing 'tools' field".to_string())
        })?;

        let tools: Vec<ToolDefinition> = serde_json::from_value(tools_value.clone()).map_err(|e| {
            McplugError::ProtocolError(format!("Failed to parse tool definitions: {e}"))
        })?;

        Ok(tools)
    }

    async fn call_tool(
        &self,
        name: &str,
        args: serde_json::Value,
    ) -> Result<CallResult, McplugError> {
        let result = self
            .send_request(
                "tools/call",
                Some(json!({
                    "name": name,
                    "arguments": args,
                })),
            )
            .await?;

        let mut call_result: CallResult =
            serde_json::from_value(result.clone()).map_err(|e| {
                McplugError::ProtocolError(format!("Failed to parse tool call result: {e}"))
            })?;

        call_result.raw_response = Some(result);

        Ok(call_result)
    }

    async fn close(&mut self) -> Result<(), McplugError> {
        // Best-effort: send a close notification but don't fail if it errors
        let _ = self.send_notification("notifications/cancelled", None).await;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_http_without_allow_flag() {
        let result = HttpSseTransport::new(
            "http://example.com/mcp",
            &HashMap::new(),
            "test-server",
            false,
        );
        assert!(result.is_err());
        let err = result.unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("Cleartext HTTP is not allowed"), "got: {msg}");
    }

    #[test]
    fn allows_http_with_flag() {
        let result = HttpSseTransport::new(
            "http://localhost:8080/mcp",
            &HashMap::new(),
            "test-server",
            true,
        );
        assert!(result.is_ok());
    }

    #[test]
    fn allows_https() {
        let result = HttpSseTransport::new(
            "https://example.com/mcp",
            &HashMap::new(),
            "test-server",
            false,
        );
        assert!(result.is_ok());
    }

    #[test]
    fn rejects_invalid_url() {
        let result = HttpSseTransport::new(
            "not a url at all",
            &HashMap::new(),
            "test-server",
            false,
        );
        assert!(result.is_err());
        let msg = result.unwrap_err().to_string();
        assert!(msg.contains("Invalid URL"), "got: {msg}");
    }

    #[test]
    fn rejects_unsupported_scheme() {
        let result = HttpSseTransport::new(
            "ftp://example.com/mcp",
            &HashMap::new(),
            "test-server",
            false,
        );
        assert!(result.is_err());
        let msg = result.unwrap_err().to_string();
        assert!(msg.contains("Unsupported URL scheme"), "got: {msg}");
    }

    #[test]
    fn custom_headers_included() {
        let mut headers = HashMap::new();
        headers.insert("Authorization".to_string(), "Bearer tok123".to_string());
        headers.insert("X-Custom".to_string(), "value".to_string());
        let transport =
            HttpSseTransport::new("https://example.com/mcp", &headers, "test-server", false)
                .unwrap();
        // Verify the transport was created successfully with custom headers
        assert_eq!(transport.base_url, "https://example.com/mcp");
        assert_eq!(transport.server_name, "test-server");
    }

    #[test]
    fn rejects_invalid_header_value() {
        let mut headers = HashMap::new();
        headers.insert("Bad-Header".to_string(), "value\r\ninjection".to_string());
        let result =
            HttpSseTransport::new("https://example.com/mcp", &headers, "test-server", false);
        assert!(result.is_err());
    }

    #[test]
    fn json_rpc_request_format() {
        let builder = RequestBuilder::new();
        let req = builder.next_request(
            "tools/call",
            Some(json!({
                "name": "my_tool",
                "arguments": {"key": "value"}
            })),
        );
        let serialized = serde_json::to_value(&req).unwrap();
        assert_eq!(serialized["jsonrpc"], "2.0");
        assert_eq!(serialized["method"], "tools/call");
        assert_eq!(serialized["params"]["name"], "my_tool");
        assert!(serialized["id"].is_u64());
    }

    #[test]
    fn session_id_starts_none() {
        let transport = HttpSseTransport::new(
            "https://example.com/mcp",
            &HashMap::new(),
            "test-server",
            false,
        )
        .unwrap();
        let guard = transport.session_id.lock().unwrap();
        assert!(guard.is_none());
    }
}
