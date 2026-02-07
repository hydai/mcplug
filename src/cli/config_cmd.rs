// NOTE: src/cli/mod.rs needs: pub mod config_cmd;

use std::collections::HashMap;
use std::io::{self, BufRead, Write};
use std::path::PathBuf;

use colored::Colorize;

use crate::config::loader::{discover_config_files, load_config, strip_jsonc_comments};
use crate::config::types::{Lifecycle, McplugConfig, ServerConfig};
use crate::error::McplugError;

/// Holds a server config together with the file it was first defined in.
struct AnnotatedEntry {
    name: String,
    config: ServerConfig,
    source: PathBuf,
}

/// Load configs with source annotations.
///
/// Walks the discovered config files in precedence order and records, for each
/// server name, the first file it appeared in.  The merged config from
/// `load_config` is authoritative for the actual values (env expansion, editor
/// imports, etc.), but we need the per-file walk to map server -> source.
fn load_annotated(cli_config: Option<&str>) -> Result<Vec<AnnotatedEntry>, McplugError> {
    let config_files = discover_config_files(cli_config);

    // Track which server came from which file (first occurrence wins).
    let mut source_map: HashMap<String, PathBuf> = HashMap::new();
    for path in &config_files {
        let content = match std::fs::read_to_string(path) {
            Ok(c) => c,
            Err(_) => continue,
        };
        let stripped = strip_jsonc_comments(&content);
        let cfg: McplugConfig = match serde_json::from_str(&stripped) {
            Ok(c) => c,
            Err(_) => continue,
        };
        for name in cfg.mcp_servers.keys() {
            source_map
                .entry(name.clone())
                .or_insert_with(|| path.clone());
        }
    }

    // Get the fully-merged (env-expanded) config.
    let merged = load_config(cli_config)?;

    let mut entries: Vec<AnnotatedEntry> = merged
        .mcp_servers
        .into_iter()
        .map(|(name, config)| {
            let source = source_map
                .get(&name)
                .cloned()
                .unwrap_or_else(|| PathBuf::from("<editor-import>"));
            AnnotatedEntry {
                name,
                config,
                source,
            }
        })
        .collect();

    // Sort by name for stable output.
    entries.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(entries)
}

/// Display merged configuration with source annotations.
pub async fn run_config_show() -> Result<(), McplugError> {
    let entries = load_annotated(None)?;
    let is_tty = atty_stdout();

    if entries.is_empty() {
        println!("No MCP servers configured.");
        return Ok(());
    }

    for (i, entry) in entries.iter().enumerate() {
        if i > 0 {
            println!();
        }
        print_entry(entry, is_tty);
    }

    Ok(())
}

fn print_entry(entry: &AnnotatedEntry, is_tty: bool) {
    let name = if is_tty {
        entry.name.bold().cyan().to_string()
    } else {
        entry.name.clone()
    };
    println!("{}", name);

    // Transport type + connection details
    if let Some(ref url) = entry.config.base_url {
        let label = if is_tty {
            "Transport".dimmed().to_string()
        } else {
            "Transport".to_string()
        };
        println!("  {}: HTTP", label);
        let url_label = if is_tty {
            "URL".dimmed().to_string()
        } else {
            "URL".to_string()
        };
        println!("  {}: {}", url_label, url);
    } else if let Some(ref cmd) = entry.config.command {
        let label = if is_tty {
            "Transport".dimmed().to_string()
        } else {
            "Transport".to_string()
        };
        println!("  {}: stdio", label);
        let cmd_label = if is_tty {
            "Command".dimmed().to_string()
        } else {
            "Command".to_string()
        };
        let full_cmd = if entry.config.args.is_empty() {
            cmd.clone()
        } else {
            format!("{} {}", cmd, entry.config.args.join(" "))
        };
        println!("  {}: {}", cmd_label, full_cmd);
    }

    // Description
    if let Some(ref desc) = entry.config.description {
        let label = if is_tty {
            "Description".dimmed().to_string()
        } else {
            "Description".to_string()
        };
        println!("  {}: {}", label, desc);
    }

    // Lifecycle
    let lifecycle_str = match entry.config.lifecycle {
        Some(Lifecycle::KeepAlive) => "keep-alive",
        Some(Lifecycle::Ephemeral) => "ephemeral",
        None => "ephemeral (default)",
    };
    let lc_label = if is_tty {
        "Lifecycle".dimmed().to_string()
    } else {
        "Lifecycle".to_string()
    };
    println!("  {}: {}", lc_label, lifecycle_str);

    // Source file
    let src_label = if is_tty {
        "Source".dimmed().to_string()
    } else {
        "Source".to_string()
    };
    println!("  {}: {}", src_label, entry.source.display());
}

/// Interactive wizard to add a new server definition.
pub async fn run_config_add() -> Result<(), McplugError> {
    let stdin = io::stdin();
    let mut reader = stdin.lock();

    let name = prompt(&mut reader, "Server name: ")?;
    if name.is_empty() {
        return Err(McplugError::ConfigError {
            path: PathBuf::from("<stdin>"),
            detail: "Server name cannot be empty".into(),
        });
    }

    let transport = prompt_choice(
        &mut reader,
        "Transport type [stdio/http]: ",
        &["stdio", "http"],
    )?;

    let mut server = ServerConfig {
        description: None,
        base_url: None,
        command: None,
        args: vec![],
        env: HashMap::new(),
        headers: HashMap::new(),
        lifecycle: None,
    };

    match transport.as_str() {
        "stdio" => {
            let cmd = prompt(&mut reader, "Command (e.g. npx): ")?;
            if cmd.is_empty() {
                return Err(McplugError::ConfigError {
                    path: PathBuf::from("<stdin>"),
                    detail: "Command cannot be empty for stdio transport".into(),
                });
            }
            let args_str = prompt(&mut reader, "Arguments (space-separated, or empty): ")?;
            let args: Vec<String> = if args_str.is_empty() {
                vec![]
            } else {
                args_str.split_whitespace().map(String::from).collect()
            };
            server.command = Some(cmd);
            server.args = args;
        }
        "http" => {
            let url = prompt(&mut reader, "Base URL: ")?;
            if url.is_empty() {
                return Err(McplugError::ConfigError {
                    path: PathBuf::from("<stdin>"),
                    detail: "Base URL cannot be empty for HTTP transport".into(),
                });
            }
            server.base_url = Some(url);
        }
        _ => unreachable!(),
    }

    let desc = prompt(&mut reader, "Description (optional, press Enter to skip): ")?;
    if !desc.is_empty() {
        server.description = Some(desc);
    }

    let lifecycle = prompt_choice(
        &mut reader,
        "Lifecycle [ephemeral/keep-alive] (default: ephemeral): ",
        &["ephemeral", "keep-alive", ""],
    )?;
    server.lifecycle = match lifecycle.as_str() {
        "keep-alive" => Some(Lifecycle::KeepAlive),
        "ephemeral" => Some(Lifecycle::Ephemeral),
        _ => None, // empty -> default
    };

    // Write to ~/.mcplug/mcplug.json
    let config_path = default_config_path()?;
    write_server_to_config(&config_path, &name, &server)?;

    println!("Server '{}' added to {}", name, config_path.display());
    Ok(())
}

fn default_config_path() -> Result<PathBuf, McplugError> {
    let home = dirs::home_dir().ok_or_else(|| McplugError::ConfigError {
        path: PathBuf::from("~"),
        detail: "Cannot determine home directory".into(),
    })?;
    Ok(home.join(".mcplug").join("mcplug.json"))
}

/// Read or create the config file, merge the new server into it, and write back.
fn write_server_to_config(
    path: &PathBuf,
    name: &str,
    server: &ServerConfig,
) -> Result<(), McplugError> {
    // Ensure parent directory exists.
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    // Read existing config or start fresh.
    let mut doc: serde_json::Value = if path.exists() {
        let content = std::fs::read_to_string(path)?;
        let stripped = strip_jsonc_comments(&content);
        serde_json::from_str(&stripped).unwrap_or_else(|_| serde_json::json!({}))
    } else {
        serde_json::json!({})
    };

    // Ensure mcpServers object exists.
    if doc.get("mcpServers").is_none() {
        doc.as_object_mut()
            .unwrap()
            .insert("mcpServers".into(), serde_json::json!({}));
    }

    let servers = doc
        .get_mut("mcpServers")
        .unwrap()
        .as_object_mut()
        .unwrap();

    let server_value = serde_json::to_value(server).map_err(|e| McplugError::ConfigError {
        path: path.clone(),
        detail: format!("Failed to serialize server config: {}", e),
    })?;

    servers.insert(name.to_string(), server_value);

    let json_str =
        serde_json::to_string_pretty(&doc).map_err(|e| McplugError::ConfigError {
            path: path.clone(),
            detail: format!("Failed to serialize config: {}", e),
        })?;

    std::fs::write(path, json_str + "\n")?;
    Ok(())
}

fn prompt(reader: &mut impl BufRead, message: &str) -> Result<String, McplugError> {
    print!("{}", message);
    io::stdout().flush()?;
    let mut line = String::new();
    reader.read_line(&mut line)?;
    Ok(line.trim().to_string())
}

fn prompt_choice(
    reader: &mut impl BufRead,
    message: &str,
    choices: &[&str],
) -> Result<String, McplugError> {
    loop {
        let answer = prompt(reader, message)?;
        let lower = answer.to_lowercase();
        if choices.contains(&lower.as_str()) {
            return Ok(lower);
        }
        println!(
            "Invalid choice. Please enter one of: {}",
            choices
                .iter()
                .filter(|c| !c.is_empty())
                .cloned()
                .collect::<Vec<_>>()
                .join(", ")
        );
    }
}

fn atty_stdout() -> bool {
    std::io::IsTerminal::is_terminal(&std::io::stdout())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    // --- show formatting tests ---

    #[test]
    fn print_entry_stdio_server() {
        let entry = AnnotatedEntry {
            name: "test-server".into(),
            config: ServerConfig {
                description: Some("A test server".into()),
                base_url: None,
                command: Some("npx".into()),
                args: vec!["-y".into(), "some-pkg".into()],
                env: HashMap::new(),
                headers: HashMap::new(),
                lifecycle: Some(Lifecycle::Ephemeral),
            },
            source: PathBuf::from("/home/user/.mcplug/mcplug.json"),
        };
        // non-TTY mode to avoid ANSI codes in test output
        print_entry(&entry, false);
    }

    #[test]
    fn print_entry_http_server() {
        let entry = AnnotatedEntry {
            name: "web-scraper".into(),
            config: ServerConfig {
                description: None,
                base_url: Some("https://mcp.example.com/mcp".into()),
                command: None,
                args: vec![],
                env: HashMap::new(),
                headers: HashMap::new(),
                lifecycle: Some(Lifecycle::KeepAlive),
            },
            source: PathBuf::from("./config/mcplug.json"),
        };
        print_entry(&entry, false);
    }

    #[test]
    fn print_entry_default_lifecycle() {
        let entry = AnnotatedEntry {
            name: "minimal".into(),
            config: ServerConfig {
                description: None,
                base_url: None,
                command: Some("echo".into()),
                args: vec![],
                env: HashMap::new(),
                headers: HashMap::new(),
                lifecycle: None,
            },
            source: PathBuf::from("<editor-import>"),
        };
        print_entry(&entry, false);
    }

    // --- config file writing tests ---

    #[test]
    fn write_server_creates_new_file() {
        let dir = tempfile::tempdir().unwrap();
        let config_path = dir.path().join("mcplug.json");

        let server = ServerConfig {
            description: Some("test desc".into()),
            base_url: None,
            command: Some("echo".into()),
            args: vec!["hello".into()],
            env: HashMap::new(),
            headers: HashMap::new(),
            lifecycle: Some(Lifecycle::Ephemeral),
        };

        write_server_to_config(&config_path, "my-server", &server).unwrap();

        assert!(config_path.exists());
        let content = std::fs::read_to_string(&config_path).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&content).unwrap();

        let servers = parsed.get("mcpServers").unwrap().as_object().unwrap();
        assert!(servers.contains_key("my-server"));
        let s = servers.get("my-server").unwrap();
        assert_eq!(s.get("command").unwrap().as_str(), Some("echo"));
        assert_eq!(s.get("description").unwrap().as_str(), Some("test desc"));
    }

    #[test]
    fn write_server_merges_into_existing_file() {
        let dir = tempfile::tempdir().unwrap();
        let config_path = dir.path().join("mcplug.json");

        // Write initial server.
        let existing = serde_json::json!({
            "mcpServers": {
                "existing": {
                    "command": "old-cmd",
                    "args": []
                }
            }
        });
        std::fs::write(
            &config_path,
            serde_json::to_string_pretty(&existing).unwrap(),
        )
        .unwrap();

        let new_server = ServerConfig {
            description: None,
            base_url: Some("https://example.com".into()),
            command: None,
            args: vec![],
            env: HashMap::new(),
            headers: HashMap::new(),
            lifecycle: Some(Lifecycle::KeepAlive),
        };

        write_server_to_config(&config_path, "new-server", &new_server).unwrap();

        let content = std::fs::read_to_string(&config_path).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&content).unwrap();
        let servers = parsed.get("mcpServers").unwrap().as_object().unwrap();

        // Both servers should be present.
        assert!(servers.contains_key("existing"));
        assert!(servers.contains_key("new-server"));
        assert_eq!(
            servers
                .get("new-server")
                .unwrap()
                .get("baseUrl")
                .unwrap()
                .as_str(),
            Some("https://example.com")
        );
    }

    #[test]
    fn write_server_creates_parent_dirs() {
        let dir = tempfile::tempdir().unwrap();
        let config_path = dir.path().join("sub").join("dir").join("mcplug.json");

        let server = ServerConfig {
            description: None,
            base_url: None,
            command: Some("test".into()),
            args: vec![],
            env: HashMap::new(),
            headers: HashMap::new(),
            lifecycle: None,
        };

        write_server_to_config(&config_path, "srv", &server).unwrap();
        assert!(config_path.exists());
    }

    // --- prompt tests ---

    #[test]
    fn prompt_reads_trimmed_line() {
        let input = b"  hello world  \n";
        let mut reader = Cursor::new(input);
        let result = prompt(&mut reader, "test: ").unwrap();
        assert_eq!(result, "hello world");
    }

    #[test]
    fn prompt_choice_accepts_valid() {
        let input = b"stdio\n";
        let mut reader = Cursor::new(input);
        let result = prompt_choice(&mut reader, "choice: ", &["stdio", "http"]).unwrap();
        assert_eq!(result, "stdio");
    }

    #[test]
    fn prompt_choice_case_insensitive() {
        let input = b"HTTP\n";
        let mut reader = Cursor::new(input);
        let result = prompt_choice(&mut reader, "choice: ", &["stdio", "http"]).unwrap();
        assert_eq!(result, "http");
    }

    #[test]
    fn prompt_choice_retries_on_invalid() {
        // First line is invalid, second is valid
        let input = b"invalid\nstdio\n";
        let mut reader = Cursor::new(input);
        let result = prompt_choice(&mut reader, "choice: ", &["stdio", "http"]).unwrap();
        assert_eq!(result, "stdio");
    }

    // --- load_annotated tests ---

    #[test]
    fn load_annotated_with_temp_config() {
        let dir = tempfile::tempdir().unwrap();
        let config_path = dir.path().join("mcplug.json");
        std::fs::write(
            &config_path,
            r#"{
                "mcpServers": {
                    "my-srv": {
                        "command": "echo",
                        "args": ["hi"]
                    }
                }
            }"#,
        )
        .unwrap();

        let entries = load_annotated(Some(config_path.to_str().unwrap())).unwrap();
        let found = entries.iter().find(|e| e.name == "my-srv");
        assert!(found.is_some());
        let entry = found.unwrap();
        assert_eq!(entry.source, config_path);
        assert_eq!(entry.config.command.as_deref(), Some("echo"));
    }
}
