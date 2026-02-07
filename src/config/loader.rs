use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::error::McplugError;

use super::editors::import_editor_configs;
use super::env::expand_server_config;
use super::types::{McplugConfig, ServerConfig};

/// Strip JSONC comments (// line comments and /* */ block comments) from input.
pub fn strip_jsonc_comments(input: &str) -> String {
    let mut result = String::with_capacity(input.len());
    let mut chars = input.chars().peekable();
    let mut in_string = false;
    let mut escape_next = false;

    while let Some(ch) = chars.next() {
        if escape_next {
            escape_next = false;
            result.push(ch);
            continue;
        }

        if in_string {
            result.push(ch);
            if ch == '\\' {
                escape_next = true;
            } else if ch == '"' {
                in_string = false;
            }
            continue;
        }

        if ch == '"' {
            in_string = true;
            result.push(ch);
            continue;
        }

        if ch == '/' {
            match chars.peek() {
                Some(&'/') => {
                    // Line comment: skip until end of line
                    chars.next();
                    for c in chars.by_ref() {
                        if c == '\n' {
                            result.push('\n');
                            break;
                        }
                    }
                }
                Some(&'*') => {
                    // Block comment: skip until */
                    chars.next();
                    let mut prev = ' ';
                    for c in chars.by_ref() {
                        if prev == '*' && c == '/' {
                            break;
                        }
                        // Preserve newlines to keep line numbers stable
                        if c == '\n' {
                            result.push('\n');
                        }
                        prev = c;
                    }
                }
                _ => {
                    result.push(ch);
                }
            }
        } else {
            result.push(ch);
        }
    }

    result
}

/// Discover config files in precedence order (highest first).
///
/// Precedence:
/// 1. `--config` CLI flag
/// 2. `MCPLUG_CONFIG` env var
/// 3. `./config/mcplug.json` (project-level)
/// 4. `~/.mcplug/mcplug.json` or `~/.mcplug/mcplug.jsonc` (home-level)
/// 5. Fallback: `~/.mcporter/mcporter.json[c]`, `./config/mcporter.json`
pub fn discover_config_files(cli_config: Option<&str>) -> Vec<PathBuf> {
    let mut files = Vec::new();

    // 1. CLI flag
    if let Some(path) = cli_config {
        let p = PathBuf::from(path);
        if p.exists() {
            files.push(p);
        }
    }

    // 2. MCPLUG_CONFIG env var
    if let Ok(env_path) = std::env::var("MCPLUG_CONFIG") {
        let p = PathBuf::from(&env_path);
        if p.exists() && !files.contains(&p) {
            files.push(p);
        }
    }

    // 3. ./config/mcplug.json (project-level)
    let project_config = PathBuf::from("./config/mcplug.json");
    if project_config.exists() && !files.contains(&project_config) {
        files.push(project_config);
    }

    // 4. ~/.mcplug/mcplug.json or ~/.mcplug/mcplug.jsonc
    if let Some(home) = dirs::home_dir() {
        let home_json = home.join(".mcplug").join("mcplug.json");
        let home_jsonc = home.join(".mcplug").join("mcplug.jsonc");
        if home_json.exists() && !files.contains(&home_json) {
            files.push(home_json);
        } else if home_jsonc.exists() && !files.contains(&home_jsonc) {
            files.push(home_jsonc);
        }
    }

    // 5. Fallback: mcporter configs
    if let Some(home) = dirs::home_dir() {
        let mcporter_json = home.join(".mcporter").join("mcporter.json");
        let mcporter_jsonc = home.join(".mcporter").join("mcporter.jsonc");
        if mcporter_json.exists() && !files.contains(&mcporter_json) {
            files.push(mcporter_json);
        } else if mcporter_jsonc.exists() && !files.contains(&mcporter_jsonc) {
            files.push(mcporter_jsonc);
        }
    }
    let mcporter_project = PathBuf::from("./config/mcporter.json");
    if mcporter_project.exists() && !files.contains(&mcporter_project) {
        files.push(mcporter_project);
    }

    files
}

/// Load a single config file, stripping JSONC comments before parsing.
fn load_config_file(path: &Path) -> Result<McplugConfig, McplugError> {
    let content = std::fs::read_to_string(path).map_err(|e| McplugError::ConfigError {
        path: path.to_path_buf(),
        detail: format!("Cannot read file: {}", e),
    })?;

    let stripped = strip_jsonc_comments(&content);
    serde_json::from_str::<McplugConfig>(&stripped).map_err(|e| McplugError::ConfigError {
        path: path.to_path_buf(),
        detail: format!("Invalid JSON: {}", e),
    })
}

/// Merge server configs from `source` into `target`.
/// Servers already present in `target` are NOT overridden (earlier sources win).
fn merge_servers(
    target: &mut HashMap<String, ServerConfig>,
    source: HashMap<String, ServerConfig>,
) {
    for (name, config) in source {
        target.entry(name).or_insert(config);
    }
}

/// Load, merge, and expand all configuration.
///
/// - Discovers config files in precedence order
/// - Merges mcpServers (earlier sources win for same name)
/// - Collects imports from all configs
/// - Imports editor configs (lowest precedence)
/// - Expands environment variables in all server configs
pub fn load_config(cli_config: Option<&str>) -> Result<McplugConfig, McplugError> {
    let config_files = discover_config_files(cli_config);

    let mut merged_servers: HashMap<String, ServerConfig> = HashMap::new();
    let mut all_imports: Vec<String> = Vec::new();

    for path in &config_files {
        let cfg = load_config_file(path)?;
        merge_servers(&mut merged_servers, cfg.mcp_servers);
        for import in cfg.imports {
            if !all_imports.contains(&import) {
                all_imports.push(import);
            }
        }
    }

    // Import editor configs (lowest precedence — merged after everything else)
    if !all_imports.is_empty() {
        let editor_servers = import_editor_configs(&all_imports);
        merge_servers(&mut merged_servers, editor_servers);
    }

    // Expand environment variables in all server configs
    for (_name, config) in &mut merged_servers {
        expand_server_config(config)?;
    }

    Ok(McplugConfig {
        mcp_servers: merged_servers,
        imports: all_imports,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strip_line_comments() {
        let input = r#"{
  // This is a comment
  "key": "value" // inline comment
}"#;
        let result = strip_jsonc_comments(input);
        let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
        assert_eq!(parsed["key"], "value");
    }

    #[test]
    fn strip_block_comments() {
        let input = r#"{
  /* block comment */
  "key": "value"
}"#;
        let result = strip_jsonc_comments(input);
        let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
        assert_eq!(parsed["key"], "value");
    }

    #[test]
    fn strip_multiline_block_comments() {
        let input = r#"{
  /*
   * multi-line
   * block comment
   */
  "key": "value"
}"#;
        let result = strip_jsonc_comments(input);
        let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
        assert_eq!(parsed["key"], "value");
    }

    #[test]
    fn preserve_strings_with_slashes() {
        let input = r#"{
  "url": "https://example.com/path",
  "pattern": "a//b"
}"#;
        let result = strip_jsonc_comments(input);
        let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
        assert_eq!(parsed["url"], "https://example.com/path");
        assert_eq!(parsed["pattern"], "a//b");
    }

    #[test]
    fn strip_mixed_comments() {
        let input = r#"{
  // line comment
  "a": 1, /* block */ "b": 2
  // another line comment
}"#;
        let result = strip_jsonc_comments(input);
        let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
        assert_eq!(parsed["a"], 1);
        assert_eq!(parsed["b"], 2);
    }

    #[test]
    fn no_comments() {
        let input = r#"{"key": "value"}"#;
        let result = strip_jsonc_comments(input);
        assert_eq!(result, input);
    }

    #[test]
    fn merge_servers_no_override() {
        let mut target = HashMap::new();
        target.insert(
            "server-a".into(),
            ServerConfig {
                description: Some("from target".into()),
                base_url: None,
                command: None,
                args: vec![],
                env: HashMap::new(),
                headers: HashMap::new(),
                lifecycle: None,
            },
        );

        let mut source = HashMap::new();
        source.insert(
            "server-a".into(),
            ServerConfig {
                description: Some("from source".into()),
                base_url: None,
                command: None,
                args: vec![],
                env: HashMap::new(),
                headers: HashMap::new(),
                lifecycle: None,
            },
        );
        source.insert(
            "server-b".into(),
            ServerConfig {
                description: Some("new server".into()),
                base_url: None,
                command: None,
                args: vec![],
                env: HashMap::new(),
                headers: HashMap::new(),
                lifecycle: None,
            },
        );

        merge_servers(&mut target, source);

        // server-a should still be "from target" (not overridden)
        assert_eq!(
            target.get("server-a").unwrap().description.as_deref(),
            Some("from target")
        );
        // server-b should be added
        assert_eq!(
            target.get("server-b").unwrap().description.as_deref(),
            Some("new server")
        );
    }

    #[test]
    fn discover_returns_empty_when_no_files_exist() {
        // With no CLI config and likely no config files in the test environment
        // for a non-existent path, we should get an empty vec or just the real files
        let files = discover_config_files(Some("/nonexistent/path/config.json"));
        // The CLI path doesn't exist so it won't be included
        for f in &files {
            assert!(f.exists());
        }
    }

    #[test]
    fn load_config_file_parses_jsonc() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.jsonc");
        std::fs::write(
            &path,
            r#"{
  // comment
  "mcpServers": {
    "test": {
      "command": "echo",
      "args": ["hello"]
    }
  }
}"#,
        )
        .unwrap();

        let config = load_config_file(&path).unwrap();
        assert!(config.mcp_servers.contains_key("test"));
        let server = config.mcp_servers.get("test").unwrap();
        assert_eq!(server.command.as_deref(), Some("echo"));
        assert_eq!(server.args, vec!["hello"]);
    }

    #[test]
    fn load_config_file_error_on_invalid_json() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("bad.json");
        std::fs::write(&path, "not valid json at all").unwrap();

        let err = load_config_file(&path).unwrap_err();
        assert!(err.to_string().contains("Invalid JSON"));
    }

    #[test]
    fn load_config_file_error_on_missing_file() {
        let err = load_config_file(Path::new("/nonexistent/file.json")).unwrap_err();
        assert!(err.to_string().contains("Cannot read file"));
    }
}
