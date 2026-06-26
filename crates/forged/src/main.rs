mod lifecycle;
mod logging;
mod shutdown;

use anyhow::Context;
use forge_api::{router::create_router, state::AppState};
use forge_model::Config;
use forge_store::{migrate::run_migrations, Store};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let config = Config::load().context("Failed to load config")?;

    logging::init(&config.log_level);

    tracing::info!(
        version = env!("CARGO_PKG_VERSION"),
        host = %config.host,
        port = config.port,
        "Starting forged"
    );

    // Check for stale/live lock
    match lifecycle::read_lock(&config) {
        Ok(Some(lock)) if lifecycle::is_alive(&lock) => {
            anyhow::bail!(
                "Daemon already running: pid={} port={}",
                lock.pid,
                lock.port
            );
        }
        Ok(Some(_)) => {
            tracing::warn!("Stale lock file found; taking over");
        }
        Ok(None) => {}
        Err(e) => {
            tracing::warn!(error = %e, "Could not read lock file; continuing");
        }
    }

    // Write lock file
    lifecycle::write_lock(&config).context("Failed to write lock file")?;

    // Connect to DB
    let store = Store::connect(&config)
        .await
        .context("Failed to connect to database")?;

    // Run migrations
    run_migrations(&store.pool)
        .await
        .context("Failed to run migrations")?;

    tracing::info!("Migrations applied");

    let state = AppState::new(store, config.clone());
    let router = create_router(state.clone());

    let addr = format!("{}:{}", config.host, config.port);
    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .with_context(|| format!("Failed to bind to {addr}"))?;

    tracing::info!(address = %addr, "HTTP server listening");

    let result = axum::serve(listener, router)
        .with_graceful_shutdown(shutdown::shutdown_signal())
        .await;

    // Cleanup on shutdown
    lifecycle::remove_lock(&state.config);
    tracing::info!("forged stopped");

    result.context("Server error")
}
