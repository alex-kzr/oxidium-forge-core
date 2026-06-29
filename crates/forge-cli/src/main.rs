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
        file: std::path::PathBuf,
    },
    /// Deploy a BPMN file
    Deploy {
        file: std::path::PathBuf,
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
    /// Manage jobs (worker contract)
    Jobs {
        #[command(subcommand)]
        action: JobsAction,
    },
    /// Manage incidents
    Incidents {
        #[command(subcommand)]
        action: IncidentsAction,
    },
    /// Manage manual tasks
    #[command(name = "manual-task")]
    ManualTask {
        #[command(subcommand)]
        action: ManualTaskAction,
    },
}

#[derive(Subcommand)]
enum DaemonAction {
    Start,
    Stop,
    Status,
    Restart,
}

#[derive(Subcommand)]
enum DefinitionsAction {
    List,
    Activate { key: i64 },
}

#[derive(Subcommand)]
enum InstanceAction {
    Start {
        bpmn_process_id: String,
        #[arg(long = "var", value_name = "KEY=VALUE")]
        vars: Vec<String>,
        #[arg(long)]
        variables: Option<PathBuf>,
    },
    Status {
        key: i64,
    },
}

#[derive(Subcommand)]
enum JobsAction {
    /// Fetch and lock jobs by task type
    Activate {
        #[arg(long = "type", value_name = "TASK_TYPE")]
        task_type: String,
        #[arg(long, default_value = "cli-worker")]
        worker: String,
        #[arg(long, default_value_t = 1)]
        max: i64,
        /// Lock duration in seconds
        #[arg(long, default_value_t = 60)]
        lock: i64,
    },
    /// Complete an activated job
    Complete {
        key: i64,
        #[arg(long = "var", value_name = "KEY=VALUE")]
        vars: Vec<String>,
        #[arg(long)]
        variables: Option<PathBuf>,
    },
    /// Report a job failure
    Fail {
        key: i64,
        #[arg(long)]
        error: String,
        #[arg(long, default_value_t = 0)]
        retries: i64,
        /// Retry backoff in seconds
        #[arg(long)]
        backoff: Option<i64>,
    },
}

#[derive(Subcommand)]
enum IncidentsAction {
    List {
        #[arg(long)]
        state: Option<String>,
    },
    Resolve {
        key: i64,
    },
}

#[derive(Subcommand)]
enum ManualTaskAction {
    /// List manual tasks
    List {
        #[arg(long)]
        state: Option<String>,
        #[arg(long)]
        instance: Option<i64>,
    },
    /// Complete a manual task
    Complete {
        key: i64,
        #[arg(long = "var", value_name = "KEY=VALUE")]
        vars: Vec<String>,
        #[arg(long)]
        variables: Option<PathBuf>,
    },
    /// Cancel a manual task
    Cancel {
        key: i64,
        #[arg(long)]
        reason: Option<String>,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_env("FORGE_LOG")
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("warn")),
        )
        .try_init()
        .ok();

    let mut config = Config::load().unwrap_or_else(|_| Config::default());
    config.host = cli.host.clone();
    config.port = cli.port;

    let base_url = format!("http://{}:{}", cli.host, cli.port);
    let client = RestClient::new(&base_url);

    let ensure = |p: Option<&str>| {
        let client = client.clone();
        let p = p.map(|s| s.to_string());
        async move {
            ensure::ensure_daemon_running(&client, p.as_deref()).await
        }
    };

    match cli.command {
        Commands::Health => {
            ensure(cli.config.as_deref().map(|p| p.to_str().unwrap_or(""))).await?;
            commands::health::run(&client, cli.json).await?;
        }
        Commands::Daemon { action } => match action {
            DaemonAction::Start => commands::daemon::start(&client, &config).await?,
            DaemonAction::Stop => commands::daemon::stop(&config).await?,
            DaemonAction::Status => commands::daemon::status(&client, &config, cli.json).await?,
            DaemonAction::Restart => commands::daemon::restart(&client, &config).await?,
        },
        Commands::Validate { file } => {
            ensure(cli.config.as_deref().map(|p| p.to_str().unwrap_or(""))).await?;
            commands::deploy::validate_bpmn(&client, &file, cli.json).await?;
        }
        Commands::Deploy { file, activate } => {
            ensure(cli.config.as_deref().map(|p| p.to_str().unwrap_or(""))).await?;
            commands::deploy::deploy_bpmn(&client, &file, activate, cli.json).await?;
        }
        Commands::Definitions { action } => {
            ensure(cli.config.as_deref().map(|p| p.to_str().unwrap_or(""))).await?;
            match action {
                DefinitionsAction::List => commands::definitions::list(&client, cli.json).await?,
                DefinitionsAction::Activate { key } => {
                    commands::definitions::activate(&client, key, cli.json).await?
                }
            }
        }
        Commands::Instance { action } => {
            ensure(cli.config.as_deref().map(|p| p.to_str().unwrap_or(""))).await?;
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
        Commands::Jobs { action } => {
            ensure(cli.config.as_deref().map(|p| p.to_str().unwrap_or(""))).await?;
            match action {
                JobsAction::Activate {
                    task_type,
                    worker,
                    max,
                    lock,
                } => {
                    commands::jobs::activate(&client, &task_type, &worker, max, lock, cli.json)
                        .await?;
                }
                JobsAction::Complete {
                    key,
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
                    commands::jobs::complete(&client, key, &kv_pairs, variables.as_deref(), cli.json)
                        .await?;
                }
                JobsAction::Fail {
                    key,
                    error,
                    retries,
                    backoff,
                } => {
                    commands::jobs::fail(&client, key, &error, retries, backoff, cli.json).await?;
                }
            }
        }
        Commands::Incidents { action } => {
            ensure(cli.config.as_deref().map(|p| p.to_str().unwrap_or(""))).await?;
            match action {
                IncidentsAction::List { state } => {
                    commands::incidents::list(&client, state.as_deref(), cli.json).await?;
                }
                IncidentsAction::Resolve { key } => {
                    commands::incidents::resolve(&client, key, cli.json).await?;
                }
            }
        }
        Commands::ManualTask { action } => {
            ensure(cli.config.as_deref().map(|p| p.to_str().unwrap_or(""))).await?;
            match action {
                ManualTaskAction::List { state, instance } => {
                    commands::manual_task::list(&client, state.as_deref(), instance, cli.json)
                        .await?;
                }
                ManualTaskAction::Complete {
                    key,
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
                    commands::manual_task::complete(
                        &client,
                        key,
                        &kv_pairs,
                        variables.as_deref(),
                        cli.json,
                    )
                    .await?;
                }
                ManualTaskAction::Cancel { key, reason } => {
                    commands::manual_task::cancel(&client, key, reason.as_deref(), cli.json)
                        .await?;
                }
            }
        }
    }

    Ok(())
}
