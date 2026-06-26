use tracing_subscriber::{EnvFilter, fmt};

pub fn init(log_level: &str) {
    let filter = EnvFilter::try_from_env("FORGE_LOG")
        .or_else(|_| EnvFilter::try_from_env("RUST_LOG"))
        .unwrap_or_else(|_| EnvFilter::new(log_level));

    fmt().with_env_filter(filter).try_init().ok();
}
