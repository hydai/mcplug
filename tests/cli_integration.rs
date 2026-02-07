use assert_cmd::Command;
use predicates::prelude::*;

mod common;

fn mcplug_cmd() -> Command {
    Command::cargo_bin("mcplug").unwrap()
}

/// I7: mcplug list <server> output
#[test]
fn list_server_tools_output() {
    let config_dir = common::temp_config_dir(&common::mock_stdio_config("mock"));
    let config_path = config_dir.path().join("mcplug.json");
    mcplug_cmd()
        .args(["list", "mock"])
        .env("MCPLUG_CONFIG", &config_path)
        .assert()
        .success()
        .stdout(predicate::str::contains("add"));
}

/// I8: mcplug list --json
#[test]
fn list_json_output() {
    let config_dir = common::temp_config_dir(&common::mock_stdio_config("mock"));
    let config_path = config_dir.path().join("mcplug.json");
    mcplug_cmd()
        .args(["list", "mock", "--json"])
        .env("MCPLUG_CONFIG", &config_path)
        .assert()
        .success()
        .stdout(predicate::str::contains("\"toolCount\""));
}

/// I9: mcplug call with colon args
#[test]
fn call_tool_colon_args() {
    let config_dir = common::temp_config_dir(&common::mock_stdio_config("mock"));
    let config_path = config_dir.path().join("mcplug.json");
    mcplug_cmd()
        .args(["call", "mock.add", "a:3", "b:4"])
        .env("MCPLUG_CONFIG", &config_path)
        .assert()
        .success()
        .stdout(predicate::str::contains("7"));
}

/// I10: mcplug call function syntax
#[test]
fn call_tool_function_syntax() {
    let config_dir = common::temp_config_dir(&common::mock_stdio_config("mock"));
    let config_path = config_dir.path().join("mcplug.json");
    mcplug_cmd()
        .args(["call", "mock.add(a: 10, b: 20)"])
        .env("MCPLUG_CONFIG", &config_path)
        .assert()
        .success()
        .stdout(predicate::str::contains("30"));
}

/// I11: mcplug call JSON output
#[test]
fn call_tool_json_output() {
    let config_dir = common::temp_config_dir(&common::mock_stdio_config("mock"));
    let config_path = config_dir.path().join("mcplug.json");
    mcplug_cmd()
        .args(["call", "mock.echo", "input:hello", "--json"])
        .env("MCPLUG_CONFIG", &config_path)
        .assert()
        .success()
        .stdout(predicate::str::contains("content"))
        .stdout(predicate::str::contains("isError"));
}

/// I12: mcplug call tool not found
#[test]
fn call_tool_not_found_error() {
    let config_dir = common::temp_config_dir(&common::mock_stdio_config("mock"));
    let config_path = config_dir.path().join("mcplug.json");
    mcplug_cmd()
        .args(["call", "mock.nonexistent_tool"])
        .env("MCPLUG_CONFIG", &config_path)
        .assert()
        .failure();
}

/// I13: mcplug call near-miss suggestion
#[test]
fn call_tool_near_miss_suggestion() {
    let config_dir = common::temp_config_dir(&common::mock_stdio_config("mock"));
    let config_path = config_dir.path().join("mcplug.json");
    mcplug_cmd()
        .args(["call", "mock.ech"])
        .env("MCPLUG_CONFIG", &config_path)
        .assert()
        .failure()
        .stderr(predicate::str::contains("echo").or(predicate::str::contains("not found")));
}

/// I14: mcplug config show
#[test]
fn config_show_output() {
    let config_dir = common::temp_config_dir(&common::mock_stdio_config("mock"));
    let config_path = config_dir.path().join("mcplug.json");
    mcplug_cmd()
        .args(["config", "show"])
        .env("MCPLUG_CONFIG", &config_path)
        .assert()
        .success()
        .stdout(predicate::str::contains("mock"));
}
