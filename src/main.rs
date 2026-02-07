use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "mcplug", version, about = "A toolkit for discovering, calling, and composing MCP servers")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// List configured MCP servers and their tools
    List {
        /// Server name to list tools for
        server: Option<String>,

        /// Query an ad-hoc HTTP endpoint
        #[arg(long)]
        http_url: Option<String>,

        /// Query an ad-hoc stdio server
        #[arg(long)]
        stdio: Option<String>,

        /// Output in JSON format
        #[arg(long)]
        json: bool,

        /// Show all parameters including optional ones
        #[arg(long)]
        all_parameters: bool,
    },

    /// Call an MCP tool
    Call {
        /// Tool reference in server.tool format
        tool_ref: String,

        /// Tool arguments
        args: Vec<String>,

        /// Raw output (no formatting)
        #[arg(long)]
        raw: bool,

        /// JSON output
        #[arg(long)]
        json: bool,

        /// Output format
        #[arg(long)]
        output: Option<String>,

        /// Ad-hoc HTTP endpoint
        #[arg(long)]
        http_url: Option<String>,

        /// Ad-hoc stdio server
        #[arg(long)]
        stdio: Option<String>,
    },

    /// Complete OAuth login for a protected MCP server
    Auth {
        /// Server name or URL
        server: String,

        /// OAuth timeout in milliseconds
        #[arg(long, env = "MCPLUG_OAUTH_TIMEOUT_MS")]
        oauth_timeout: Option<u64>,
    },

    /// Manage persistent background servers
    Daemon {
        #[command(subcommand)]
        action: DaemonAction,
    },

    /// Generate a standalone CLI binary for a specific MCP server
    #[command(name = "generate-cli")]
    GenerateCli {
        /// Server name
        server: String,

        /// Compile the generated source
        #[arg(long)]
        compile: bool,

        /// Include only specified tools
        #[arg(long, value_delimiter = ',')]
        include_tools: Option<Vec<String>>,

        /// Exclude specified tools
        #[arg(long, value_delimiter = ',')]
        exclude_tools: Option<Vec<String>>,
    },

    /// Emit Rust type definitions and client wrappers for an MCP server
    #[command(name = "emit-rs")]
    EmitRs {
        /// Server name
        server: String,

        /// Output file path
        #[arg(long)]
        output: Option<String>,
    },

    /// Manage server configuration
    Config {
        #[command(subcommand)]
        action: ConfigAction,
    },
}

#[derive(Subcommand)]
enum DaemonAction {
    /// Start daemon for keep-alive servers
    Start {
        /// Server name (optional, starts all keep-alive servers if omitted)
        server: Option<String>,

        /// Enable detailed logging
        #[arg(long)]
        log: bool,
    },
    /// Stop running daemon
    Stop {
        /// Server name (optional)
        server: Option<String>,
    },
    /// Restart daemon
    Restart {
        /// Server name (optional)
        server: Option<String>,
    },
    /// Show daemon status
    Status,
}

#[derive(Subcommand)]
enum ConfigAction {
    /// Add a new server definition interactively
    Add,
    /// Display merged config with source annotations
    Show,
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_env("MCPLUG_LOG_LEVEL")
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("warn")),
        )
        .with_writer(std::io::stderr)
        .init();

    let cli = Cli::parse();

    let result = run(cli).await;
    if let Err(e) = result {
        eprintln!("Error: {e}");
        std::process::exit(1);
    }
}

async fn run(cli: Cli) -> Result<(), mcplug::McplugError> {
    match cli.command {
        Commands::List {
            server,
            http_url,
            stdio,
            json,
            all_parameters,
        } => {
            mcplug::cli::list::run_list(
                server.as_deref(),
                http_url.as_deref(),
                stdio.as_deref(),
                json,
                all_parameters,
            )
            .await
        }
        Commands::Call {
            tool_ref,
            args,
            raw,
            json,
            output,
            http_url,
            stdio,
        } => {
            mcplug::cli::call::run_call(
                &tool_ref,
                &args,
                raw,
                json,
                output.as_deref(),
                http_url.as_deref(),
                stdio.as_deref(),
            )
            .await
        }
        Commands::Auth {
            server,
            oauth_timeout,
        } => {
            let timeout = std::time::Duration::from_millis(oauth_timeout.unwrap_or(60000));
            // Determine base_url from config or treat server as URL
            let config = mcplug::load_config(None)?;
            let base_url = if server.starts_with("http://") || server.starts_with("https://") {
                server.clone()
            } else {
                let srv = config.mcp_servers.get(&server).ok_or_else(|| {
                    mcplug::McplugError::ServerNotFound(server.clone())
                })?;
                srv.base_url.clone().ok_or_else(|| {
                    mcplug::McplugError::ConfigError {
                        path: std::path::PathBuf::from("<config>"),
                        detail: format!("Server '{}' has no baseUrl for OAuth", server),
                    }
                })?
            };
            let token = mcplug::oauth::flow::run_oauth_flow(&base_url, &server, timeout).await?;
            println!("Authentication successful for '{}'", server);
            println!("Token expires: {:?}", token.expires_at);
            Ok(())
        }
        Commands::Daemon { action } => {
            let dm = mcplug::daemon::DaemonManager::new();
            match action {
                DaemonAction::Start { server, log } => {
                    dm.start(server.as_deref(), log).await
                }
                DaemonAction::Stop { server } => {
                    dm.stop(server.as_deref()).await
                }
                DaemonAction::Restart { server } => {
                    dm.restart(server.as_deref(), false).await
                }
                DaemonAction::Status => {
                    let status = dm.status().await?;
                    if status.running {
                        println!("Daemon running (PID: {})", status.pid.unwrap_or(0));
                        println!("Managed servers: {:?}", status.managed_servers);
                    } else {
                        println!("Daemon is not running");
                    }
                    Ok(())
                }
            }
        }
        Commands::GenerateCli {
            server,
            compile,
            include_tools,
            exclude_tools,
        } => {
            let runtime = mcplug::Runtime::from_config().await?;
            let tools = runtime.list_tools(&server).await?;
            let source = mcplug::codegen::generate_cli::generate_cli_source(
                &tools,
                &server,
                include_tools.as_deref(),
                exclude_tools.as_deref(),
            );
            if compile {
                let dir = std::env::temp_dir().join(format!("mcplug-gen-{}", server));
                std::fs::create_dir_all(&dir).map_err(mcplug::McplugError::IoError)?;
                let main_path = dir.join("main.rs");
                std::fs::write(&main_path, &source).map_err(mcplug::McplugError::IoError)?;
                println!("Generated source at: {}", main_path.display());
                eprintln!("Compilation requires a Cargo project setup — source written to {}", main_path.display());
            } else {
                println!("{source}");
            }
            Ok(())
        }
        Commands::EmitRs { server, output } => {
            let runtime = mcplug::Runtime::from_config().await?;
            let tools = runtime.list_tools(&server).await?;
            let code = mcplug::codegen::emit_rs::emit_rust_types(&tools, &server);
            if let Some(path) = output {
                std::fs::write(&path, &code).map_err(mcplug::McplugError::IoError)?;
                println!("Wrote Rust types to {path}");
            } else {
                println!("{code}");
            }
            Ok(())
        }
        Commands::Config { action } => match action {
            ConfigAction::Add => mcplug::cli::config_cmd::run_config_add().await,
            ConfigAction::Show => mcplug::cli::config_cmd::run_config_show().await,
        },
    }
}
