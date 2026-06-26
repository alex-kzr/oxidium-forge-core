use anyhow::{bail, Context, Result};
use std::time::{Duration, Instant};

use crate::client::RestClient;

const POLL_INTERVAL: Duration = Duration::from_millis(500);
const BOOT_TIMEOUT: Duration = Duration::from_secs(30);

/// Ensures the daemon is running, starting it if necessary.
/// Returns when the daemon is healthy or after exhausting retries.
pub async fn ensure_daemon_running(client: &RestClient, config_path: Option<&str>) -> Result<()> {
    if client.is_healthy().await {
        return Ok(());
    }

    tracing::info!("Daemon not running; attempting to start it");

    start_daemon_detached(config_path).context("Failed to spawn forged")?;

    let deadline = Instant::now() + BOOT_TIMEOUT;
    while Instant::now() < deadline {
        tokio::time::sleep(POLL_INTERVAL).await;
        if client.is_healthy().await {
            tracing::info!("Daemon is healthy");
            return Ok(());
        }
    }

    bail!(
        "Daemon did not become healthy within {}s",
        BOOT_TIMEOUT.as_secs()
    );
}

fn start_daemon_detached(config_path: Option<&str>) -> Result<()> {
    let forged = forged_exe_path()?;
    tracing::debug!(?forged, "Spawning daemon");

    let mut cmd = std::process::Command::new(&forged);
    if let Some(cfg) = config_path {
        cmd.args(["--config", cfg]);
    }

    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        // DETACHED_PROCESS | CREATE_NO_WINDOW
        cmd.creation_flags(0x00000008 | 0x08000000);
    }

    cmd.spawn().with_context(|| format!("Failed to spawn {forged:?}"))?;
    Ok(())
}

fn forged_exe_path() -> Result<std::path::PathBuf> {
    // Look for forged next to the current binary first.
    let mut path = std::env::current_exe().context("Cannot determine current exe")?;
    path.pop();
    path.push("forged");

    #[cfg(windows)]
    path.set_extension("exe");

    if path.exists() {
        return Ok(path);
    }

    // Fall back to PATH lookup.
    which::which("forged").context("forged not found on PATH")
}
