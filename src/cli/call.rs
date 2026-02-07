use std::io::IsTerminal;
use std::time::Duration;

use crate::args::{parse_args, parse_function_call, parse_tool_ref, suggest_tool};
use crate::config::load_config;
use crate::error::McplugError;

use super::connection::connect_to_server;
use super::output::{print_call_result, OutputMode};

/// Default timeout for call operations.
const DEFAULT_TIMEOUT_SECS: u64 = 30;

/// Get the call timeout from the environment variable or use the default.
fn get_timeout() -> Duration {
    parse_timeout_secs(std::env::var("MCPLUG_CALL_TIMEOUT").ok())
}

fn parse_timeout_secs(val: Option<String>) -> Duration {
    val.and_then(|v| v.parse::<u64>().ok())
        .map(Duration::from_secs)
        .unwrap_or(Duration::from_secs(DEFAULT_TIMEOUT_SECS))
}

/// Determine the output mode from CLI flags.
fn resolve_output_mode(raw: bool, json: bool, output_format: Option<&str>) -> OutputMode {
    if json {
        return OutputMode::Json;
    }
    if raw {
        return OutputMode::Raw;
    }
    match output_format {
        Some("json") => OutputMode::Json,
        Some("raw") => OutputMode::Raw,
        _ => OutputMode::Pretty,
    }
}

/// Run the call command.
pub async fn run_call(
    tool_ref: &str,
    args: &[String],
    raw: bool,
    json: bool,
    output_format: Option<&str>,
    http_url: Option<&str>,
    stdio: Option<&str>,
) -> Result<(), McplugError> {
    let config = load_config(None)?;
    let timeout = get_timeout();
    let mode = resolve_output_mode(raw, json, output_format);
    let is_tty = std::io::stdout().is_terminal();

    // Parse tool reference: support both "server.tool" and "server.tool(args)" syntax
    let (server_name, tool_name, parsed_args) = if tool_ref.contains('(') {
        let (s, t, a) = parse_function_call(tool_ref)?;
        (s, t, a)
    } else {
        let (s, t) = parse_tool_ref(tool_ref)?;
        let a = parse_args(args)?;
        (s, t, a)
    };

    // Connect and initialize
    let mut transport = connect_to_server(&server_name, &config, http_url, stdio)?;

    let result = tokio::time::timeout(timeout, async {
        transport.initialize().await?;

        // Validate tool name exists and provide suggestions if not found
        let tools = transport.list_tools().await?;
        let tool_names: Vec<&str> = tools.iter().map(|t| t.name.as_str()).collect();

        if !tool_names.contains(&tool_name.as_str()) {
            let suggestion = suggest_tool(&tool_name, &tool_names);
            let mut msg = format!(
                "Tool '{}' not found on {}.",
                tool_name, server_name
            );
            if let Some(ref s) = suggestion {
                msg.push_str(&format!(" Did you mean '{}'?", s));
            }
            return Err(McplugError::ToolNotFound {
                server: server_name.clone(),
                tool: tool_name.clone(),
            });
        }

        transport.call_tool(&tool_name, parsed_args).await
    })
    .await
    .map_err(|_| McplugError::Timeout {
        server: server_name.clone(),
        tool: Some(tool_name.clone()),
        duration: timeout,
    })??;

    print_call_result(&result, mode, is_tty);

    let _ = transport.close().await;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_output_json_flag() {
        assert_eq!(resolve_output_mode(false, true, None), OutputMode::Json);
    }

    #[test]
    fn resolve_output_raw_flag() {
        assert_eq!(resolve_output_mode(true, false, None), OutputMode::Raw);
    }

    #[test]
    fn resolve_output_format_json() {
        assert_eq!(
            resolve_output_mode(false, false, Some("json")),
            OutputMode::Json
        );
    }

    #[test]
    fn resolve_output_format_raw() {
        assert_eq!(
            resolve_output_mode(false, false, Some("raw")),
            OutputMode::Raw
        );
    }

    #[test]
    fn resolve_output_default_pretty() {
        assert_eq!(resolve_output_mode(false, false, None), OutputMode::Pretty);
    }

    #[test]
    fn resolve_output_json_takes_priority_over_raw() {
        assert_eq!(resolve_output_mode(true, true, None), OutputMode::Json);
    }

    #[test]
    fn timeout_default() {
        assert_eq!(parse_timeout_secs(None), Duration::from_secs(DEFAULT_TIMEOUT_SECS));
    }

    #[test]
    fn timeout_from_value() {
        assert_eq!(parse_timeout_secs(Some("120".into())), Duration::from_secs(120));
    }

    #[test]
    fn timeout_invalid_value() {
        assert_eq!(parse_timeout_secs(Some("bad".into())), Duration::from_secs(DEFAULT_TIMEOUT_SECS));
    }

    #[test]
    fn resolve_output_format_unknown_falls_to_pretty() {
        assert_eq!(
            resolve_output_mode(false, false, Some("unknown")),
            OutputMode::Pretty
        );
    }

    #[test]
    fn timeout_zero_is_valid() {
        assert_eq!(parse_timeout_secs(Some("0".into())), Duration::from_secs(0));
    }

    #[test]
    fn timeout_negative_falls_to_default() {
        // Negative number cannot parse as u64, so falls back to default
        assert_eq!(
            parse_timeout_secs(Some("-1".into())),
            Duration::from_secs(DEFAULT_TIMEOUT_SECS)
        );
    }

    #[test]
    fn resolve_output_format_empty_string_falls_to_pretty() {
        assert_eq!(
            resolve_output_mode(false, false, Some("")),
            OutputMode::Pretty
        );
    }

    #[test]
    fn resolve_output_format_case_sensitive() {
        // "Json" (capitalized) should not match "json", so falls to Pretty
        assert_eq!(
            resolve_output_mode(false, false, Some("Json")),
            OutputMode::Pretty
        );
        assert_eq!(
            resolve_output_mode(false, false, Some("RAW")),
            OutputMode::Pretty
        );
    }

    #[test]
    fn timeout_very_large_value() {
        assert_eq!(
            parse_timeout_secs(Some("999999".into())),
            Duration::from_secs(999999)
        );
    }
}
