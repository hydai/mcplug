use crate::types::ToolDefinition;
use std::fmt::Write;

use super::emit_rs::{json_schema_to_rust_type, sanitize_identifier, to_pascal_case, to_snake_case};

/// Generate a standalone CLI Rust source file from MCP tool definitions.
///
/// The generated code depends on `mcplug`, `clap`, `serde`, `serde_json`, and `tokio`.
pub fn generate_cli_source(
    tools: &[ToolDefinition],
    server_name: &str,
    include: Option<&[String]>,
    exclude: Option<&[String]>,
) -> String {
    let filtered_tools = filter_tools(tools, include, exclude);
    let mut out = String::new();

    // File header
    writeln!(out, "// Auto-generated CLI by mcplug. Do not edit.").unwrap();
    writeln!(out).unwrap();
    writeln!(out, "use clap::{{Parser, Subcommand}};").unwrap();
    writeln!(out, "use serde::{{Deserialize, Serialize}};").unwrap();
    writeln!(out).unwrap();

    // Generate arg structs for each tool
    for tool in &filtered_tools {
        let struct_name = format!("{}Args", to_pascal_case(&tool.name));
        emit_clap_args_struct(&mut out, &struct_name, tool);
        writeln!(out).unwrap();
    }

    // Generate the subcommand enum
    let app_name = to_pascal_case(server_name);
    writeln!(out, "#[derive(Debug, Subcommand)]").unwrap();
    writeln!(out, "pub enum Commands {{").unwrap();
    for tool in &filtered_tools {
        let variant = to_pascal_case(&tool.name);
        let args_type = format!("{variant}Args");
        // Add doc comment from tool description
        if !tool.description.is_empty() {
            writeln!(out, "    /// {}", tool.description).unwrap();
        }
        writeln!(out, "    {variant}({args_type}),").unwrap();
    }
    writeln!(out, "}}").unwrap();
    writeln!(out).unwrap();

    // Generate the CLI parser struct
    writeln!(out, "#[derive(Debug, Parser)]").unwrap();
    writeln!(
        out,
        "#[command(name = \"{server_name}\", about = \"CLI for {app_name} MCP server\")]"
    )
    .unwrap();
    writeln!(out, "pub struct Cli {{").unwrap();
    writeln!(out, "    #[command(subcommand)]").unwrap();
    writeln!(out, "    pub command: Commands,").unwrap();
    writeln!(out, "}}").unwrap();
    writeln!(out).unwrap();

    // Generate main function
    writeln!(out, "#[tokio::main]").unwrap();
    writeln!(out, "async fn main() -> Result<(), Box<dyn std::error::Error>> {{").unwrap();
    writeln!(out, "    let cli = Cli::parse();").unwrap();
    writeln!(out).unwrap();
    writeln!(
        out,
        "    let config = mcplug::load_config(None)?;"
    )
    .unwrap();
    writeln!(out, "    let runtime = mcplug::Runtime::new(config).await?;").unwrap();
    writeln!(out).unwrap();
    writeln!(out, "    let result = match cli.command {{").unwrap();
    for tool in &filtered_tools {
        let variant = to_pascal_case(&tool.name);
        let args_var = to_snake_case(&tool.name);
        writeln!(out, "        Commands::{variant}({args_var}) => {{").unwrap();
        writeln!(
            out,
            "            runtime.call_tool(\"{server}\", \"{tool}\", serde_json::to_value({args_var})?).await?",
            server = server_name,
            tool = tool.name,
        )
        .unwrap();
        writeln!(out, "        }}").unwrap();
    }
    writeln!(out, "    }};").unwrap();
    writeln!(out).unwrap();
    writeln!(out, "    println!(\"{{}}\", result.text());").unwrap();
    writeln!(out, "    Ok(())").unwrap();
    writeln!(out, "}}").unwrap();

    out
}

fn filter_tools<'a>(
    tools: &'a [ToolDefinition],
    include: Option<&[String]>,
    exclude: Option<&[String]>,
) -> Vec<&'a ToolDefinition> {
    tools
        .iter()
        .filter(|tool| {
            if let Some(include_list) = include {
                if !include_list.contains(&tool.name) {
                    return false;
                }
            }
            if let Some(exclude_list) = exclude {
                if exclude_list.contains(&tool.name) {
                    return false;
                }
            }
            true
        })
        .collect()
}

fn emit_clap_args_struct(out: &mut String, name: &str, tool: &ToolDefinition) {
    let schema = &tool.input_schema;

    writeln!(out, "#[derive(Debug, Clone, Serialize, Deserialize, clap::Args)]").unwrap();
    writeln!(out, "pub struct {name} {{").unwrap();

    let required = schema
        .get("required")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str())
                .map(String::from)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    if let Some(props) = schema.get("properties").and_then(|v| v.as_object()) {
        for (prop_name, prop_schema) in props {
            let field_name = sanitize_identifier(&to_snake_case(prop_name));
            let rust_type = json_schema_to_rust_type(prop_schema);

            // Add description as doc comment
            if let Some(desc) = prop_schema.get("description").and_then(|v| v.as_str()) {
                writeln!(out, "    /// {desc}").unwrap();
            }

            // Add serde rename if field name differs
            if field_name != *prop_name {
                writeln!(out, "    #[serde(rename = \"{prop_name}\")]").unwrap();
            }

            if required.contains(prop_name) {
                writeln!(out, "    pub {field_name}: {rust_type},").unwrap();
            } else {
                writeln!(out, "    pub {field_name}: Option<{rust_type}>,").unwrap();
            }
        }
    }

    writeln!(out, "}}").unwrap();
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::ToolDefinition;

    fn sample_tools() -> Vec<ToolDefinition> {
        vec![
            ToolDefinition {
                name: "get-weather".to_string(),
                description: "Get weather for a location".to_string(),
                input_schema: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "location": {"type": "string", "description": "City name"},
                        "units": {"type": "string"}
                    },
                    "required": ["location"]
                }),
            },
            ToolDefinition {
                name: "set-alarm".to_string(),
                description: "Set an alarm".to_string(),
                input_schema: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "time": {"type": "string"},
                        "repeat": {"type": "boolean"}
                    },
                    "required": ["time"]
                }),
            },
        ]
    }

    #[test]
    fn test_generate_cli_source_basic() {
        let tools = sample_tools();
        let output = generate_cli_source(&tools, "my-service", None, None);

        assert!(output.contains("use clap::{Parser, Subcommand};"));
        assert!(output.contains("pub struct GetWeatherArgs"));
        assert!(output.contains("pub struct SetAlarmArgs"));
        assert!(output.contains("pub enum Commands"));
        assert!(output.contains("GetWeather(GetWeatherArgs)"));
        assert!(output.contains("SetAlarm(SetAlarmArgs)"));
        assert!(output.contains("pub struct Cli"));
        assert!(output.contains("async fn main()"));
        assert!(output.contains("Commands::GetWeather"));
        assert!(output.contains("Commands::SetAlarm"));
    }

    #[test]
    fn test_generate_cli_source_include_filter() {
        let tools = sample_tools();
        let include = vec!["get-weather".to_string()];
        let output = generate_cli_source(&tools, "svc", Some(&include), None);

        assert!(output.contains("GetWeather"));
        assert!(!output.contains("SetAlarm"));
    }

    #[test]
    fn test_generate_cli_source_exclude_filter() {
        let tools = sample_tools();
        let exclude = vec!["set-alarm".to_string()];
        let output = generate_cli_source(&tools, "svc", None, Some(&exclude));

        assert!(output.contains("GetWeather"));
        assert!(!output.contains("SetAlarm"));
    }

    #[test]
    fn test_generate_cli_source_include_and_exclude() {
        let tools = sample_tools();
        let include = vec!["get-weather".to_string(), "set-alarm".to_string()];
        let exclude = vec!["set-alarm".to_string()];
        let output = generate_cli_source(&tools, "svc", Some(&include), Some(&exclude));

        assert!(output.contains("GetWeather"));
        assert!(!output.contains("SetAlarm"));
    }

    #[test]
    fn test_generate_cli_source_empty_tools() {
        let output = generate_cli_source(&[], "empty", None, None);
        assert!(output.contains("pub enum Commands"));
        assert!(output.contains("async fn main()"));
    }

    #[test]
    fn test_generate_cli_required_and_optional_fields() {
        let tools = sample_tools();
        let output = generate_cli_source(&tools, "svc", None, None);

        // location is required
        assert!(output.contains("pub location: String,"));
        // units is optional
        assert!(output.contains("pub units: Option<String>,"));
    }

    #[test]
    fn test_generate_cli_tool_descriptions_as_docs() {
        let tools = sample_tools();
        let output = generate_cli_source(&tools, "svc", None, None);

        assert!(output.contains("/// Get weather for a location"));
        assert!(output.contains("/// Set an alarm"));
    }

    #[test]
    fn test_filter_tools_no_filters() {
        let tools = sample_tools();
        let filtered = filter_tools(&tools, None, None);
        assert_eq!(filtered.len(), 2);
    }

    #[test]
    fn test_filter_tools_include_only() {
        let tools = sample_tools();
        let include = vec!["get-weather".to_string()];
        let filtered = filter_tools(&tools, Some(&include), None);
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].name, "get-weather");
    }

    #[test]
    fn test_filter_tools_exclude_only() {
        let tools = sample_tools();
        let exclude = vec!["get-weather".to_string()];
        let filtered = filter_tools(&tools, None, Some(&exclude));
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].name, "set-alarm");
    }

    #[test]
    fn test_filter_tools_exclude_all() {
        let tools = sample_tools();
        let exclude = vec!["get-weather".to_string(), "set-alarm".to_string()];
        let filtered = filter_tools(&tools, None, Some(&exclude));
        assert_eq!(filtered.len(), 0);
    }

    #[test]
    fn test_generate_cli_nested_array_items() {
        let tools = vec![ToolDefinition {
            name: "list-items".to_string(),
            description: "List items with tags".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "tags": {
                        "type": "array",
                        "items": {"type": "string"}
                    },
                    "ids": {
                        "type": "array",
                        "items": {"type": "integer"}
                    }
                },
                "required": ["tags"]
            }),
        }];
        let output = generate_cli_source(&tools, "svc", None, None);
        assert!(output.contains("pub tags: Vec<String>"));
        assert!(output.contains("pub ids: Option<Vec<i64>>"));
    }
}
