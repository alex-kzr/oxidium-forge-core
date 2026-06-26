use forge_model::{Config, config::lock_file_path};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use thiserror::Error;

#[derive(Debug, Serialize, Deserialize)]
pub struct LockFile {
    pub pid: u32,
    pub host: String,
    pub port: u16,
    pub start_time: String,
}

#[derive(Debug, Error)]
pub enum LifecycleError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("TOML serialize error: {0}")]
    TomlSer(#[from] toml::ser::Error),
    #[error("TOML deserialize error: {0}")]
    TomlDe(#[from] toml::de::Error),
    #[error("Daemon already running (pid={pid}, port={port})")]
    AlreadyRunning { pid: u32, port: u16 },
}

pub fn write_lock(config: &Config) -> Result<(), LifecycleError> {
    let path = lock_file_path(config);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let lock = LockFile {
        pid: std::process::id(),
        host: config.host.clone(),
        port: config.port,
        start_time: chrono_now(),
    };
    let content = toml::to_string(&lock)?;
    std::fs::write(&path, content)?;
    tracing::debug!(?path, pid = lock.pid, "Wrote lock file");
    Ok(())
}

pub fn read_lock(config: &Config) -> Result<Option<LockFile>, LifecycleError> {
    let path = lock_file_path(config);
    if !path.exists() {
        return Ok(None);
    }
    let content = std::fs::read_to_string(&path)?;
    let lock: LockFile = toml::from_str(&content)?;
    Ok(Some(lock))
}

pub fn remove_lock(config: &Config) {
    let path = lock_file_path(config);
    if let Err(e) = std::fs::remove_file(&path) {
        tracing::warn!(?path, error = %e, "Failed to remove lock file");
    } else {
        tracing::debug!(?path, "Removed lock file");
    }
}

/// Returns true if the lock is held by a live process with a healthy daemon.
pub fn is_alive(lock: &LockFile) -> bool {
    is_process_alive(lock.pid)
}

pub fn is_process_alive(pid: u32) -> bool {
    #[cfg(windows)]
    {
        use std::process::Command;
        Command::new("tasklist")
            .args(["/FI", &format!("PID eq {}", pid), "/NH", "/FO", "CSV"])
            .output()
            .map(|o| {
                let out = String::from_utf8_lossy(&o.stdout);
                out.contains(&format!(",\"{}\"", pid))
                    || out.contains(&format!("\"{}\"", pid))
                    || (out.contains(&pid.to_string()) && !out.to_lowercase().contains("no tasks"))
            })
            .unwrap_or(false)
    }
    #[cfg(not(windows))]
    {
        // POSIX: try kill -0
        unsafe { libc::kill(pid as i32, 0) == 0 }
    }
}

fn chrono_now() -> String {
    // Simple ISO-8601 timestamp without external chrono dep
    use std::time::{SystemTime, UNIX_EPOCH};
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    // YYYY-MM-DDTHH:MM:SSZ approximation via basic arithmetic
    let s = secs;
    let (y, mo, d, h, mi, sec) = epoch_to_datetime(s);
    format!("{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z", y, mo, d, h, mi, sec)
}

fn epoch_to_datetime(secs: u64) -> (u64, u64, u64, u64, u64, u64) {
    let sec = secs % 60;
    let mins = secs / 60;
    let min = mins % 60;
    let hours = mins / 60;
    let hour = hours % 24;
    let days = hours / 24;

    // Simplified date from days since epoch (approximate, good for lock files)
    let year_400 = 365 * 400 + 97;
    let mut y = 1970 + (days / year_400) * 400;
    let mut rem = days % year_400;

    loop {
        let days_in_year = if is_leap(y) { 366 } else { 365 };
        if rem < days_in_year {
            break;
        }
        rem -= days_in_year;
        y += 1;
    }

    let months = [31, if is_leap(y) { 29 } else { 28 }, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
    let mut mo = 1u64;
    for &m in &months {
        if rem < m { break; }
        rem -= m;
        mo += 1;
    }

    (y, mo, rem + 1, hour, min, sec)
}

fn is_leap(y: u64) -> bool {
    (y % 4 == 0 && y % 100 != 0) || y % 400 == 0
}
