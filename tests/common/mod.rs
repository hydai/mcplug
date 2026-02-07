pub mod http_mock;

use std::collections::HashMap;
use std::path::PathBuf;
use mcplug::config::{McplugConfig, ServerConfig};

/// Build a McplugConfig pointing at the mock stdio server binary.
pub fn mock_stdio_config(server_name: &str) -> McplugConfig {
    let mock_bin = mock_server_bin_path();
    let mut servers = HashMap::new();
    servers.insert(
        server_name.to_string(),
        ServerConfig {
            description: Some("Mock MCP server".into()),
            base_url: None,
            command: Some(mock_bin.to_string_lossy().into_owned()),
            args: vec![],
            env: HashMap::new(),
            headers: HashMap::new(),
            lifecycle: None,
        },
    );
    McplugConfig {
        mcp_servers: servers,
        imports: vec![],
    }
}

/// Path to the compiled mock server binary.
pub fn mock_server_bin_path() -> PathBuf {
    let mut path = PathBuf::from(env!("CARGO_TARGET_TMPDIR"));
    path.pop(); // up from tmp
    path.push("debug");
    path.push("mock_mcp_server");
    path
}

/// Create a temp directory with a mcplug.json config file.
#[allow(dead_code)]
pub fn temp_config_dir(config: &McplugConfig) -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    let config_path = dir.path().join("mcplug.json");
    let json = serde_json::to_string_pretty(config).unwrap();
    std::fs::write(config_path, json).unwrap();
    dir
}
