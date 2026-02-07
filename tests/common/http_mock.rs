use wiremock::{MockServer, Mock, ResponseTemplate};
use wiremock::matchers::{method, body_partial_json};

/// Start a mock MCP HTTP server that responds to initialize + tools/list + tools/call.
#[allow(dead_code)]
pub async fn start_mock_http_server() -> MockServer {
    let server = MockServer::start().await;

    // Initialize endpoint
    Mock::given(method("POST"))
        .and(body_partial_json(serde_json::json!({
            "method": "initialize"
        })))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "result": {
                "protocolVersion": "2025-03-26",
                "serverInfo": { "name": "mock-http", "version": "1.0.0" },
                "capabilities": { "tools": {} }
            }
        })))
        .mount(&server)
        .await;

    server
}
