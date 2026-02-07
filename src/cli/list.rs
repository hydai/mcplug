use std::time::Duration;

use colored::Colorize;

use crate::config::load_config;
use crate::error::McplugError;
use crate::types::ToolDefinition;

use super::connection::connect_to_server;

/// Default timeout for list operations.
const DEFAULT_TIMEOUT_SECS: u64 = 30;

/// Get the list timeout from the environment variable or use the default.
fn get_timeout() -> Duration {
    parse_timeout_secs(std::env::var("MCPLUG_LIST_TIMEOUT").ok())
}

fn parse_timeout_secs(val: Option<String>) -> Duration {
    val.and_then(|v| v.parse::<u64>().ok())
        .map(Duration::from_secs)
        .unwrap_or(Duration::from_secs(DEFAULT_TIMEOUT_SECS))
}

/// Format a tool definition as a function signature string.
fn format_tool_signature(tool: &ToolDefinition, all_parameters: bool) -> String {
    let schema = &tool.input_schema;
    let properties = schema.get("properties").and_then(|p| p.as_object());
    let required: Vec<&str> = schema
        .get("required")
        .and_then(|r| r.as_array())
        .map(|arr| arr.iter().filter_map(|v| v.as_str()).collect())
        .unwrap_or_default();

    let mut params = Vec::new();

    if let Some(props) = properties {
        let required_count = required.len();

        for (name, prop_schema) in props {
            let is_required = required.contains(&name.as_str());
            let show_optional = all_parameters || required_count < 5;

            if !is_required && !show_optional {
                continue;
            }

            let type_str = prop_schema
                .get("type")
                .and_then(|t| t.as_str())
                .unwrap_or("any");

            if is_required {
                params.push(format!("{}: {}", name, type_str));
            } else {
                params.push(format!("{}?: {}", name, type_str));
            }
        }
    }

    format!("{}({})", tool.name, params.join(", "))
}

/// Run the list command.
pub async fn run_list(
    server: Option<&str>,
    http_url: Option<&str>,
    stdio: Option<&str>,
    json: bool,
    all_parameters: bool,
) -> Result<(), McplugError> {
    let config = load_config(None)?;
    let timeout = get_timeout();
    let is_tty = std::io::stdout().is_terminal();

    match server {
        Some(name) => {
            // List tools for a specific server
            list_server_tools(name, &config, http_url, stdio, json, all_parameters, timeout, is_tty).await
        }
        None if http_url.is_some() || stdio.is_some() => {
            // Ad-hoc server without a name
            list_server_tools("adhoc", &config, http_url, stdio, json, all_parameters, timeout, is_tty).await
        }
        None => {
            // List all configured servers
            list_all_servers(&config, json, timeout, is_tty).await
        }
    }
}

/// List tools for a specific server.
async fn list_server_tools(
    server_name: &str,
    config: &crate::config::McplugConfig,
    http_url: Option<&str>,
    stdio: Option<&str>,
    json: bool,
    all_parameters: bool,
    timeout: Duration,
    is_tty: bool,
) -> Result<(), McplugError> {
    let mut transport = connect_to_server(server_name, config, http_url, stdio)?;

    let tools = tokio::time::timeout(timeout, async {
        transport.initialize().await?;
        transport.list_tools().await
    })
    .await
    .map_err(|_| McplugError::Timeout {
        server: server_name.to_string(),
        tool: None,
        duration: timeout,
    })??;

    if json {
        let json_output = serde_json::json!({
            "server": server_name,
            "tools": tools.iter().map(|t| {
                serde_json::json!({
                    "name": t.name,
                    "description": t.description,
                    "inputSchema": t.input_schema,
                })
            }).collect::<Vec<_>>(),
            "toolCount": tools.len(),
        });
        println!(
            "{}",
            serde_json::to_string_pretty(&json_output).unwrap_or_default()
        );
    } else {
        for tool in &tools {
            let sig = format_tool_signature(tool, all_parameters);
            if is_tty {
                println!("  {}", sig.bold());
            } else {
                println!("  {}", sig);
            }
            if !tool.description.is_empty() {
                if is_tty {
                    println!("    {}", tool.description.dimmed());
                } else {
                    println!("    {}", tool.description);
                }
            }
        }
    }

    let _ = transport.close().await;
    Ok(())
}

/// List all configured servers with connection status.
async fn list_all_servers(
    config: &crate::config::McplugConfig,
    json: bool,
    timeout: Duration,
    is_tty: bool,
) -> Result<(), McplugError> {
    if config.mcp_servers.is_empty() {
        if json {
            println!("{}", serde_json::json!({"servers": [], "total": 0, "reachable": 0, "unreachable": 0}));
        } else {
            eprintln!("No servers configured.");
        }
        return Ok(());
    }

    let mut server_names: Vec<&String> = config.mcp_servers.keys().collect();
    server_names.sort();

    let mut results = Vec::new();

    for name in &server_names {
        let status = match connect_to_server(name, config, None, None) {
            Ok(mut transport) => {
                match tokio::time::timeout(timeout, transport.initialize()).await {
                    Ok(Ok(info)) => {
                        let _ = transport.close().await;
                        ServerStatus {
                            name: name.to_string(),
                            reachable: true,
                            version: Some(info.version),
                            tool_count: None,
                            error: None,
                        }
                    }
                    Ok(Err(e)) => ServerStatus {
                        name: name.to_string(),
                        reachable: false,
                        version: None,
                        tool_count: None,
                        error: Some(e.to_string()),
                    },
                    Err(_) => ServerStatus {
                        name: name.to_string(),
                        reachable: false,
                        version: None,
                        tool_count: None,
                        error: Some(format!("Timeout after {}s", timeout.as_secs())),
                    },
                }
            }
            Err(e) => ServerStatus {
                name: name.to_string(),
                reachable: false,
                version: None,
                tool_count: None,
                error: Some(e.to_string()),
            },
        };
        results.push(status);
    }

    if json {
        let reachable = results.iter().filter(|s| s.reachable).count();
        let unreachable = results.iter().filter(|s| !s.reachable).count();
        let json_output = serde_json::json!({
            "servers": results.iter().map(|s| {
                let mut obj = serde_json::json!({
                    "name": s.name,
                    "status": if s.reachable { "ok" } else { "error" },
                });
                if let Some(ref v) = s.version {
                    obj["version"] = serde_json::json!(v);
                }
                if let Some(ref e) = s.error {
                    obj["error"] = serde_json::json!(e);
                }
                obj
            }).collect::<Vec<_>>(),
            "total": results.len(),
            "reachable": reachable,
            "unreachable": unreachable,
        });
        println!(
            "{}",
            serde_json::to_string_pretty(&json_output).unwrap_or_default()
        );
    } else {
        for status in &results {
            if status.reachable {
                let label = if is_tty {
                    "ok".green().to_string()
                } else {
                    "ok".to_string()
                };
                let version_str = status
                    .version
                    .as_deref()
                    .map(|v| format!(" (v{})", v))
                    .unwrap_or_default();
                println!("  {} [{}]{}", status.name, label, version_str);
            } else {
                let label = if is_tty {
                    "error".red().to_string()
                } else {
                    "error".to_string()
                };
                let err_str = status
                    .error
                    .as_deref()
                    .map(|e| format!(" - {}", e))
                    .unwrap_or_default();
                println!("  {} [{}]{}", status.name, label, err_str);
            }
        }
    }

    Ok(())
}

struct ServerStatus {
    name: String,
    reachable: bool,
    version: Option<String>,
    #[allow(dead_code)]
    tool_count: Option<usize>,
    error: Option<String>,
}

use std::io::IsTerminal;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::ToolDefinition;

    fn make_tool(name: &str, desc: &str, schema: serde_json::Value) -> ToolDefinition {
        ToolDefinition {
            name: name.to_string(),
            description: desc.to_string(),
            input_schema: schema,
        }
    }

    #[test]
    fn format_tool_no_params() {
        let tool = make_tool("ping", "Ping server", serde_json::json!({}));
        let sig = format_tool_signature(&tool, false);
        assert_eq!(sig, "ping()");
    }

    #[test]
    fn format_tool_required_params() {
        let tool = make_tool(
            "search",
            "Search things",
            serde_json::json!({
                "type": "object",
                "properties": {
                    "query": {"type": "string"},
                    "limit": {"type": "integer"},
                },
                "required": ["query"],
            }),
        );
        let sig = format_tool_signature(&tool, false);
        // With < 5 required params, optional params are shown
        assert!(sig.contains("query: string"));
        assert!(sig.contains("limit?: integer"));
    }

    #[test]
    fn format_tool_hides_optional_when_many_required() {
        let tool = make_tool(
            "complex",
            "Complex tool",
            serde_json::json!({
                "type": "object",
                "properties": {
                    "a": {"type": "string"},
                    "b": {"type": "string"},
                    "c": {"type": "string"},
                    "d": {"type": "string"},
                    "e": {"type": "string"},
                    "optional_one": {"type": "string"},
                },
                "required": ["a", "b", "c", "d", "e"],
            }),
        );
        let sig = format_tool_signature(&tool, false);
        assert!(sig.contains("a: string"));
        assert!(!sig.contains("optional_one"));
    }

    #[test]
    fn format_tool_shows_optional_when_all_params_flag() {
        let tool = make_tool(
            "complex",
            "Complex tool",
            serde_json::json!({
                "type": "object",
                "properties": {
                    "a": {"type": "string"},
                    "b": {"type": "string"},
                    "c": {"type": "string"},
                    "d": {"type": "string"},
                    "e": {"type": "string"},
                    "optional_one": {"type": "string"},
                },
                "required": ["a", "b", "c", "d", "e"],
            }),
        );
        let sig = format_tool_signature(&tool, true);
        assert!(sig.contains("optional_one"));
    }

    #[test]
    fn timeout_default() {
        assert_eq!(parse_timeout_secs(None), Duration::from_secs(DEFAULT_TIMEOUT_SECS));
    }

    #[test]
    fn timeout_from_value() {
        assert_eq!(parse_timeout_secs(Some("60".into())), Duration::from_secs(60));
    }

    #[test]
    fn timeout_invalid_value() {
        assert_eq!(parse_timeout_secs(Some("not_a_number".into())), Duration::from_secs(DEFAULT_TIMEOUT_SECS));
    }
}
