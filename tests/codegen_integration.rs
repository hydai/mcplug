mod common;

use mcplug::codegen::generate_cli::generate_cli_source;
use mcplug::codegen::emit_rs::emit_rust_types;

/// I19: Generate CLI from mock server
#[tokio::test]
async fn generate_cli_from_mock_server() {
    let config = common::mock_stdio_config("mock");
    let runtime = mcplug::Runtime::with_config(config);
    let tools = runtime.list_tools("mock").await.unwrap();
    let source = generate_cli_source(&tools, "mock", None, None);
    assert!(source.contains("pub struct AddArgs"));
    assert!(source.contains("pub struct EchoArgs"));
    assert!(source.contains("pub enum Commands"));
    assert!(source.contains("async fn main()"));
    runtime.close().await.unwrap();
}

/// I20: Generate CLI with tool filter
#[tokio::test]
async fn generate_cli_with_tool_filter() {
    let config = common::mock_stdio_config("mock");
    let runtime = mcplug::Runtime::with_config(config);
    let tools = runtime.list_tools("mock").await.unwrap();
    let include = vec!["add".to_string(), "echo".to_string()];
    let source = generate_cli_source(&tools, "mock", Some(&include), None);
    assert!(source.contains("Add"));
    assert!(source.contains("Echo"));
    assert!(!source.contains("Slow"));
    assert!(!source.contains("Counter"));
    runtime.close().await.unwrap();
}

/// I21: Generated Rust types are valid
#[tokio::test]
async fn generated_rust_types_are_valid() {
    let config = common::mock_stdio_config("mock");
    let runtime = mcplug::Runtime::with_config(config);
    let tools = runtime.list_tools("mock").await.unwrap();
    let source = emit_rust_types(&tools, "mock");
    assert!(source.contains("use serde::{Deserialize, Serialize};"));
    assert!(source.contains("#[derive(Debug, Clone, Serialize, Deserialize)]"));
    assert!(source.contains("pub struct MockClient"));
    for tool in &tools {
        let pascal = tool
            .name
            .chars()
            .next()
            .unwrap()
            .to_uppercase()
            .to_string()
            + &tool.name[1..];
        assert!(
            source.contains(&format!("{}Args", pascal)) || source.contains("Args"),
            "Missing args struct for tool: {}",
            tool.name
        );
    }
    runtime.close().await.unwrap();
}
