use std::collections::HashMap;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};

use async_trait::async_trait;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader, BufWriter};
use tokio::process::{Child, ChildStdin, ChildStdout, Command};
use tokio::sync::Mutex;
use tracing::{debug, warn};

use crate::error::McplugError;
use crate::transport::McpTransport;
use crate::types::{CallResult, ServerInfo, ToolDefinition};

use super::jsonrpc::{JsonRpcResponse, RequestBuilder};

pub struct StdioTransport {
    child: Mutex<Child>,
    stdin: Mutex<BufWriter<ChildStdin>>,
    stdout: Mutex<BufReader<ChildStdout>>,
    next_id: AtomicU64,
    server_name: String,
}

impl std::fmt::Debug for StdioTransport {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("StdioTransport")
            .field("server_name", &self.server_name)
            .field("next_id", &self.next_id)
            .finish_non_exhaustive()
    }
}

impl StdioTransport {
    /// Spawn a child process and create a new StdioTransport.
    pub fn new(
        command: &str,
        args: &[String],
        env: &HashMap<String, String>,
        cwd: Option<&Path>,
        server_name: &str,
    ) -> Result<Self, McplugError> {
        let mut cmd = Command::new(command);
        cmd.args(args)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::inherit())
            .envs(env);

        if let Some(dir) = cwd {
            cmd.current_dir(dir);
        }

        let mut child = cmd.spawn().map_err(|e| McplugError::ConnectionFailed {
            server: server_name.to_string(),
            source: Box::new(e),
        })?;

        let child_stdin = child
            .stdin
            .take()
            .ok_or_else(|| McplugError::TransportError("Failed to capture stdin".into()))?;
        let child_stdout = child
            .stdout
            .take()
            .ok_or_else(|| McplugError::TransportError("Failed to capture stdout".into()))?;

        Ok(Self {
            child: Mutex::new(child),
            stdin: Mutex::new(BufWriter::new(child_stdin)),
            stdout: Mutex::new(BufReader::new(child_stdout)),
            next_id: AtomicU64::new(1),
            server_name: server_name.to_string(),
        })
    }

    /// Send a JSON-RPC request and read the response.
    async fn send_request(
        &self,
        method: &str,
        params: Option<serde_json::Value>,
    ) -> Result<JsonRpcResponse, McplugError> {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let builder = RequestBuilder::new();
        // We don't use the builder's auto-increment here; we manage IDs ourselves.
        let mut req = builder.next_request(method, params);
        req.id = id;

        let req_json = serde_json::to_string(&req).map_err(|e| {
            McplugError::ProtocolError(format!("Failed to serialize request: {e}"))
        })?;

        debug!(server = %self.server_name, method, id, "sending request");

        // Write request to stdin
        {
            let mut stdin = self.stdin.lock().await;
            stdin
                .write_all(req_json.as_bytes())
                .await
                .map_err(|e| McplugError::TransportError(Box::new(e)))?;
            stdin
                .write_all(b"\n")
                .await
                .map_err(|e| McplugError::TransportError(Box::new(e)))?;
            stdin
                .flush()
                .await
                .map_err(|e| McplugError::TransportError(Box::new(e)))?;
        }

        // Read response lines until we get one matching our request ID.
        // Skip notifications (lines without an id or with a different id).
        loop {
            let line = self.read_line().await?;
            let resp: JsonRpcResponse = serde_json::from_str(&line).map_err(|e| {
                McplugError::ProtocolError(format!(
                    "Failed to parse response: {e}\nRaw line: {line}"
                ))
            })?;

            // If this is a notification (no id), skip it
            if resp.id.is_none() {
                debug!(server = %self.server_name, "skipping notification");
                continue;
            }

            // If this response matches our request id, return it
            if resp.id == Some(id) {
                return Ok(resp);
            }

            // Unexpected id — log a warning and keep reading
            warn!(
                server = %self.server_name,
                expected_id = id,
                got_id = ?resp.id,
                "received response with unexpected id, skipping"
            );
        }
    }

    /// Send a JSON-RPC notification (no id, no response expected).
    async fn send_notification(
        &self,
        method: &str,
        params: Option<serde_json::Value>,
    ) -> Result<(), McplugError> {
        let notif = RequestBuilder::notification(method, params);
        let json = serde_json::to_string(&notif).map_err(|e| {
            McplugError::ProtocolError(format!("Failed to serialize notification: {e}"))
        })?;

        debug!(server = %self.server_name, method, "sending notification");

        let mut stdin = self.stdin.lock().await;
        stdin
            .write_all(json.as_bytes())
            .await
            .map_err(|e| McplugError::TransportError(Box::new(e)))?;
        stdin
            .write_all(b"\n")
            .await
            .map_err(|e| McplugError::TransportError(Box::new(e)))?;
        stdin
            .flush()
            .await
            .map_err(|e| McplugError::TransportError(Box::new(e)))?;

        Ok(())
    }

    /// Read a single line from stdout.
    async fn read_line(&self) -> Result<String, McplugError> {
        let mut stdout = self.stdout.lock().await;
        let mut line = String::new();
        let bytes_read = stdout
            .read_line(&mut line)
            .await
            .map_err(|e| McplugError::TransportError(Box::new(e)))?;

        if bytes_read == 0 {
            return Err(McplugError::TransportError(
                format!("Server '{}' process exited unexpectedly", self.server_name).into(),
            ));
        }

        Ok(line)
    }

    /// Check a JSON-RPC response for errors, returning the result value on success.
    fn check_response(
        &self,
        resp: JsonRpcResponse,
    ) -> Result<serde_json::Value, McplugError> {
        if let Some(err) = resp.error {
            return Err(McplugError::ProtocolError(format!(
                "JSON-RPC error {}: {}{}",
                err.code,
                err.message,
                err.data
                    .map(|d| format!(" ({})", d))
                    .unwrap_or_default()
            )));
        }

        resp.result
            .ok_or_else(|| McplugError::ProtocolError("Response missing both result and error".into()))
    }
}

#[async_trait]
impl McpTransport for StdioTransport {
    async fn initialize(&mut self) -> Result<ServerInfo, McplugError> {
        let params = serde_json::json!({
            "protocolVersion": "2024-11-05",
            "capabilities": {},
            "clientInfo": {
                "name": "mcplug",
                "version": "0.1.0"
            }
        });

        let resp = self.send_request("initialize", Some(params)).await?;
        let result = self.check_response(resp)?;

        // Extract server info from the result
        let server_info_value = result
            .get("serverInfo")
            .cloned()
            .unwrap_or_else(|| serde_json::json!({}));

        let name = server_info_value
            .get("name")
            .and_then(|v| v.as_str())
            .unwrap_or(&self.server_name)
            .to_string();

        let version = server_info_value
            .get("version")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown")
            .to_string();

        let capabilities = result
            .get("capabilities")
            .cloned()
            .unwrap_or_else(|| serde_json::json!({}));

        // Send initialized notification
        self.send_notification("notifications/initialized", None)
            .await?;

        Ok(ServerInfo {
            name,
            version,
            capabilities,
        })
    }

    async fn list_tools(&self) -> Result<Vec<ToolDefinition>, McplugError> {
        let resp = self
            .send_request("tools/list", Some(serde_json::json!({})))
            .await?;
        let result = self.check_response(resp)?;

        let tools_value = result
            .get("tools")
            .ok_or_else(|| McplugError::ProtocolError("tools/list response missing 'tools' field".into()))?;

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
        let params = serde_json::json!({
            "name": name,
            "arguments": args
        });

        let resp = self.send_request("tools/call", Some(params)).await?;
        let result = self.check_response(resp)?;

        let call_result: CallResult =
            serde_json::from_value(result.clone()).map_err(|e| {
                McplugError::ProtocolError(format!("Failed to parse call result: {e}"))
            })?;

        Ok(CallResult {
            raw_response: Some(result),
            ..call_result
        })
    }

    async fn close(&mut self) -> Result<(), McplugError> {
        let mut child = self.child.lock().await;
        // Try to kill the child process
        if let Err(e) = child.kill().await {
            // If the process already exited, that's fine
            warn!(server = %self.server_name, error = %e, "failed to kill child process");
        }
        // Wait for the process to fully exit
        let _ = child.wait().await;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stdio_transport_creation_with_echo() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let transport = StdioTransport::new(
                "cat",
                &[],
                &HashMap::new(),
                None,
                "test-server",
            );
            assert!(transport.is_ok());
            let mut t = transport.unwrap();
            // Clean up
            let _ = t.close().await;
        });
    }

    #[test]
    fn stdio_transport_creation_fails_with_bad_command() {
        let result = StdioTransport::new(
            "nonexistent_command_12345",
            &[],
            &HashMap::new(),
            None,
            "bad-server",
        );
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("bad-server"));
    }

    #[test]
    fn stdio_transport_with_env() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let mut env = HashMap::new();
            env.insert("TEST_VAR".to_string(), "test_value".to_string());
            let transport = StdioTransport::new(
                "cat",
                &[],
                &env,
                None,
                "env-server",
            );
            assert!(transport.is_ok());
            let mut t = transport.unwrap();
            let _ = t.close().await;
        });
    }

    #[test]
    fn stdio_transport_with_cwd() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let transport = StdioTransport::new(
                "cat",
                &[],
                &HashMap::new(),
                Some(Path::new("/tmp")),
                "cwd-server",
            );
            assert!(transport.is_ok());
            let mut t = transport.unwrap();
            let _ = t.close().await;
        });
    }

    #[tokio::test]
    async fn stdio_transport_send_and_receive() {
        // Use `cat` which echoes stdin to stdout — we can send a JSON-RPC
        // response-shaped message and read it back.
        let transport = StdioTransport::new(
            "cat",
            &[],
            &HashMap::new(),
            None,
            "echo-server",
        )
        .unwrap();

        // Manually write a fake response that `cat` will echo back
        let fake_response = r#"{"jsonrpc":"2.0","id":1,"result":{"serverInfo":{"name":"test","version":"1.0"},"capabilities":{}}}"#;
        {
            let mut stdin = transport.stdin.lock().await;
            stdin.write_all(fake_response.as_bytes()).await.unwrap();
            stdin.write_all(b"\n").await.unwrap();
            stdin.flush().await.unwrap();
        }

        // Now read it back
        let line = transport.read_line().await.unwrap();
        let resp: JsonRpcResponse = serde_json::from_str(&line).unwrap();
        assert_eq!(resp.id, Some(1));
        assert!(resp.result.is_some());

        let mut child = transport.child.lock().await;
        let _ = child.kill().await;
    }
}
