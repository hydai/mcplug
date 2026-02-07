mod common;

use mcplug::Runtime;

/// I1: List tools over stdio
#[tokio::test]
async fn list_tools_over_stdio() {
    let config = common::mock_stdio_config("mock");
    let runtime = Runtime::with_config(config);
    let tools = runtime.list_tools("mock").await.unwrap();
    assert!(tools.len() >= 5);
    let names: Vec<&str> = tools.iter().map(|t| t.name.as_str()).collect();
    assert!(names.contains(&"add"));
    assert!(names.contains(&"echo"));
    runtime.close().await.unwrap();
}

/// I2: Call tool (add a+b) over stdio
#[tokio::test]
async fn call_add_tool_over_stdio() {
    let config = common::mock_stdio_config("mock");
    let runtime = Runtime::with_config(config);
    let result = runtime
        .call_tool("mock", "add", serde_json::json!({"a": 3, "b": 4}))
        .await
        .unwrap();
    assert_eq!(result.text().trim(), "7");
    assert!(!result.is_error);
    runtime.close().await.unwrap();
}

/// I3: Call echo tool
#[tokio::test]
async fn call_echo_tool_over_stdio() {
    let config = common::mock_stdio_config("mock");
    let runtime = Runtime::with_config(config);
    let result = runtime
        .call_tool("mock", "echo", serde_json::json!({"input": "hello world"}))
        .await
        .unwrap();
    assert_eq!(result.text(), "hello world");
    runtime.close().await.unwrap();
}

/// I4: Connection reuse
#[tokio::test]
async fn connection_reuse_same_server() {
    let config = common::mock_stdio_config("mock");
    let runtime = Runtime::with_config(config);
    let r1 = runtime
        .call_tool("mock", "add", serde_json::json!({"a": 1, "b": 2}))
        .await
        .unwrap();
    assert_eq!(r1.text().trim(), "3");
    let r2 = runtime
        .call_tool("mock", "echo", serde_json::json!({"input": "reused"}))
        .await
        .unwrap();
    assert_eq!(r2.text(), "reused");
    runtime.close().await.unwrap();
}

/// I5: Multi-server same runtime
#[tokio::test]
async fn multi_server_same_runtime() {
    let mut config = common::mock_stdio_config("server-a");
    let mock_bin = common::mock_server_bin_path();
    config.mcp_servers.insert(
        "server-b".to_string(),
        mcplug::ServerConfig {
            description: None,
            base_url: None,
            command: Some(mock_bin.to_string_lossy().into_owned()),
            args: vec![],
            env: std::collections::HashMap::new(),
            headers: std::collections::HashMap::new(),
            lifecycle: None,
        },
    );
    let runtime = Runtime::with_config(config);
    let r1 = runtime
        .call_tool("server-a", "add", serde_json::json!({"a": 10, "b": 20}))
        .await
        .unwrap();
    assert_eq!(r1.text().trim(), "30");
    let r2 = runtime
        .call_tool(
            "server-b",
            "echo",
            serde_json::json!({"input": "from-b"}),
        )
        .await
        .unwrap();
    assert_eq!(r2.text(), "from-b");
    runtime.close().await.unwrap();
}

/// I6: Call timeout enforcement
#[tokio::test]
async fn call_timeout_enforcement() {
    use std::time::Duration;
    let config = common::mock_stdio_config("mock");
    let runtime = Runtime::with_config(config);
    let result = tokio::time::timeout(
        Duration::from_millis(100),
        runtime.call_tool("mock", "slow", serde_json::json!({"delay_ms": 5000})),
    )
    .await;
    assert!(result.is_err(), "Should have timed out");
    runtime.close().await.unwrap();
}
