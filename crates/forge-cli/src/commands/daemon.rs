use anyhow::Result;
use serde_json::json;

use crate::client::RestClient;
use crate::ensure::ensure_daemon_running;
use forge_model::Config;
use forge_model::config::lock_file_path;

/// `forge daemon start` — idempotent; auto-starts if not running.
pub async fn start(client: &RestClient, config: &Config) -> Result<()> {
    ensure_daemon_running(client, None).await?;
    println!("Daemon is running on {}", client.base_url);
    Ok(())
}

/// `forge daemon stop` — stop a running daemon.
pub async fn stop(config: &Config) -> Result<()> {
    let lock_path = lock_file_path(config);
    if !lock_path.exists() {
        println!("Daemon is not running");
        return Ok(());
    }
    let content = std::fs::read_to_string(&lock_path)?;
    let lock: toml::Value = toml::from_str(&content)?;
    let pid = lock.get("pid").and_then(|v| v.as_integer()).unwrap_or(0) as u32;

    if pid == 0 {
        println!("Cannot determine daemon PID from lock file");
        std::fs::remove_file(&lock_path)?;
        return Ok(());
    }

    kill_process(pid);
    // Wait briefly for process to exit
    for _ in 0..20 {
        tokio::time::sleep(std::time::Duration::from_millis(200)).await;
        if !lock_path.exists() {
            println!("Daemon stopped");
            return Ok(());
        }
    }
    // Remove stale lock if process is gone
    if !is_process_running(pid) {
        let _ = std::fs::remove_file(&lock_path);
        println!("Daemon stopped (lock cleaned up)");
    } else {
        println!("Warning: daemon may still be running (pid={})", pid);
    }
    Ok(())
}

/// `forge daemon status` — print status.
pub async fn status(client: &RestClient, config: &Config, json_output: bool) -> Result<()> {
    let lock_path = lock_file_path(config);
    let is_running = client.is_healthy().await;

    let (pid, port) = if lock_path.exists() {
        let content = std::fs::read_to_string(&lock_path).unwrap_or_default();
        let lock: toml::Value = toml::from_str(&content).unwrap_or(toml::Value::Table(Default::default()));
        let pid = lock.get("pid").and_then(|v| v.as_integer()).unwrap_or(0) as u32;
        let port = lock.get("port").and_then(|v| v.as_integer()).unwrap_or(0) as u16;
        (pid, port)
    } else {
        (0, config.port)
    };

    if json_output {
        println!(
            "{}",
            serde_json::to_string_pretty(&json!({
                "running": is_running,
                "pid": pid,
                "host": config.host,
                "port": port,
                "url": client.base_url,
            }))?
        );
    } else if is_running {
        println!("● Daemon running");
        if pid > 0 { println!("  PID:  {}", pid); }
        println!("  URL:  {}", client.base_url);
    } else {
        println!("○ Daemon stopped");
    }

    Ok(())
}

/// `forge daemon restart` — stop then start.
pub async fn restart(client: &RestClient, config: &Config) -> Result<()> {
    stop(config).await?;
    tokio::time::sleep(std::time::Duration::from_millis(500)).await;
    start(client, config).await
}

fn kill_process(pid: u32) {
    #[cfg(windows)]
    {
        let _ = std::process::Command::new("taskkill")
            .args(["/PID", &pid.to_string(), "/F"])
            .output();
    }
    #[cfg(not(windows))]
    {
        unsafe { libc::kill(pid as i32, libc::SIGTERM); }
    }
}

fn is_process_running(pid: u32) -> bool {
    #[cfg(windows)]
    {
        std::process::Command::new("tasklist")
            .args(["/FI", &format!("PID eq {}", pid), "/NH", "/FO", "CSV"])
            .output()
            .map(|o| {
                let out = String::from_utf8_lossy(&o.stdout);
                !out.to_lowercase().contains("no tasks")
                    && out.contains(&pid.to_string())
            })
            .unwrap_or(false)
    }
    #[cfg(not(windows))]
    {
        unsafe { libc::kill(pid as i32, 0) == 0 }
    }
}
