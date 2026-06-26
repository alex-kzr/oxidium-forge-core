mod client;
mod commands;
mod ensure;

use anyhow::Result;
use clap::{Parser, Subcommand};
use forge_model::Config;
use std::path::PathBuf;

use client::RestClient;

#[derive(Parser)]
#[command(name = "forge", about = "Oxidium Forge orchestrator CLI", version)]
struct Cli {
    /// Daemon host
    #[arg(long, global = true, env = "FORGE_HOST", default_value = "127.0.0.1")]
    host: String,

    /// Daemon port
    #[arg(long, global = true, env = "FORGE_PORT", default_value_t = 7890)]
    port: u16,

    /// Path to config file
    #[arg(long, global = true, env = "FORGE_CONFIG")]
    config: Option<PathBuf>,

    /// Output as JSON
    #[arg(long, global = true)]
    json: bool,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Daemon lifecycle management
    Daemon {
        #[command(subcommand)]
        action: DaemonAction,
    },
    /// Print daemon health status
    Health,
    /// Validate a BPMN file without deploying
    Validate {
        /// Path to .bpmn file
        file: std::path::PathBuf,
    },
    /// Deploy a BPMN file
    Deploy {
        /// Path to .bpmn file
        file: std::path::PathBuf,
        /// Automatically activate after deploy
        #[arg(long, default_value_t = true)]
        activate: bool,
    },
    /// Manage process definitions
    Definitions {
        #[command(subcommand)]
        action: DefinitionsAction,
    },
    /// Manage process instances
    Instance {
        #[command(subcommand)]
        action: InstanceAction,
    },
}

#[derive(Subcommand)]
enum DefinitionsAction {
    /// List all process definitions
    List,
    /// Activate a specific definition version
    Activate {
        /// Definition key
        key: i64,
    },
}

#[derive(Subcommand)]
enum InstanceAction {
    /// Start a new process instance
    Start {
        /// BPMN process id
        bpmn_process_id: String,
        /// Set a variable in key=value form (value parsed as JSON, fallback to string)
        #[arg(long = "var", value_name = "KEY=VALUE")]
        vars: Vec<String>,
        /// Load initial variables from a JSON file
        #[arg(long)]
        variables: Option<PathBuf>,
    },
    /// Show the status of a process instance
    Status {
        /// Instance key
        key: i64,
    },
}

#[derive(Subcommand)]
enum DaemonAction {
    /// Start the daemon (idempotent)
    Start,
    /// Stop a running daemon
    Stop,
    /// Show daemon status
    Status,
    /// Restart the daemon (stop then start)
    Restart,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    // Initialize minimal logging
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_env("FORGE_LOG")
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("warn")),
        )
        .try_init()
        .ok();

    // Build config (load from file/env, then override with CLI flags)
    let mut config = Config::load().unwrap_or_else(|_| Config::default());
    config.host = cli.host.clone();
    config.port = cli.port;

    let base_url = format!("http://{}:{}", cli.host, cli.port);
    let client = RestClient::new(&base_url);

    match cli.command {
        Commands::Health => {
            ensure::ensure_daemon_running(
                &client,
                cli.config.as_deref().map(|p| p.to_str().unwrap_or("")),
            )
            .await?;
            commands::health::run(&client, cli.json).await?;
        }
        Commands::Daemon { action } => match action {
            DaemonAction::Start => {
                commands::daemon::start(&client, &config).await?;
            }
            DaemonAction::Stop => {
                commands::daemon::stop(&config).await?;
            }
            DaemonAction::Status => {
                commands::daemon::status(&client, &config, cli.json).await?;
            }
            DaemonAction::Restart => {
                commands::daemon::restart(&client, &config).await?;
            }
        },
        Commands::Validate { file } => {
            ensure::ensure_daemon_running(
                &client,
                cli.config.as_deref().map(|p| p.to_str().unwrap_or("")),
            )
            .await?;
            commands::deploy::validate_bpmn(&client, &file, cli.json).await?;
        }
        Commands::Deploy { file, activate } => {
            ensure::ensure_daemon_running(
                &client,
                cli.config.as_deref().map(|p| p.to_str().unwrap_or("")),
            )
            .await?;
            commands::deploy::deploy_bpmn(&client, &file, activate, cli.json).await?;
        }
        Commands::Definitions { action } => {
            ensure::ensure_daemon_running(
                &client,
                cli.config.as_deref().map(|p| p.to_str().unwrap_or("")),
            )
            .await?;
            match action {
                DefinitionsAction::List => commands::definitions::list(&client, cli.json).await?,
                DefinitionsAction::Activate { key } => {
                    commands::definitions::activate(&client, key, cli.json).await?
                }
            }
        }
        Commands::Instance { action } => {
            ensure::ensure_daemon_running(
                &client,
                cli.config.as_deref().map(|p| p.to_str().unwrap_or("")),
            )
            .await?;
            match action {
                InstanceAction::Start {
                    bpmn_process_id,
                    vars,
                    variables,
                } => {
                    let kv_pairs: Vec<(String, String)> = vars
                        .into_iter()
                        .map(|s| {
                            let (k, v) = s.split_once('=').unwrap_or((&s, "null"));
                            (k.to_string(), v.to_string())
                        })
                        .collect();
                    commands::instance::start(
                        &client,
                        &bpmn_process_id,
                        &kv_pairs,
                        variables.as_deref(),
                        cli.json,
                    )
                    .await?;
                }
                InstanceAction::Status { key } => {
                    commands::instance::status(&client, key, cli.json).await?;
                }
            }
        }
    }

    Ok(())
}
