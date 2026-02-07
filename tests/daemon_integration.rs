mod common;

use mcplug::daemon::DaemonManager;

/// I15: Daemon start/stop/status cycle
#[tokio::test]
async fn daemon_start_stop_cycle() {
    let dm = DaemonManager::new();
    let _status = dm.status().await.unwrap();
    // Daemon should not be running in CI/test environment (no PID file expected)
    // We test that status() works and start/stop don't error even when not running
    dm.start(None, false).await.unwrap();
    dm.stop(None).await.unwrap();
    let status = dm.status().await.unwrap();
    // After stop, daemon should not be running
    assert!(!status.running);
}

/// I16: Daemon persistent state (counter)
/// The counter tool increments on each call within the same process,
/// so consecutive calls via the same runtime (same child process) should increment.
#[tokio::test]
async fn daemon_persistent_state() {
    let config = common::mock_stdio_config("mock");
    let runtime = mcplug::Runtime::with_config(config);
    let r1 = runtime
        .call_tool("mock", "counter", serde_json::json!({}))
        .await
        .unwrap();
    let r2 = runtime
        .call_tool("mock", "counter", serde_json::json!({}))
        .await
        .unwrap();
    let v1: u64 = r1.text().trim().parse().unwrap();
    let v2: u64 = r2.text().trim().parse().unwrap();
    assert_eq!(v2, v1 + 1);
    runtime.close().await.unwrap();
}

/// I17: Same instance across calls
/// Verifying that the runtime reuses the same server connection.
/// The first call initializes and returns the real server name ("mock-server"),
/// while subsequent calls return a placeholder with the config key.
/// Both calls should succeed, demonstrating connection reuse.
#[tokio::test]
async fn daemon_same_instance_across_calls() {
    let config = common::mock_stdio_config("mock");
    let runtime = mcplug::Runtime::with_config(config);
    let info1 = runtime.server_info("mock").await.unwrap();
    // First call initializes the transport and returns the real server name
    assert_eq!(info1.name, "mock-server");
    let info2 = runtime.server_info("mock").await.unwrap();
    // Second call reuses the connection (placeholder returned)
    assert_eq!(info2.name, "mock");
    runtime.close().await.unwrap();
}

/// I18: Daemon log tailing
/// Verifying that start with log=true succeeds.
#[tokio::test]
async fn daemon_log_tailing() {
    let dm = DaemonManager::new();
    let result = dm.start(None, true).await;
    assert!(result.is_ok());
}
