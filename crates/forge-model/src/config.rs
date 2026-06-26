use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use thiserror::Error;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub db_path: PathBuf,
    pub host: String,
    pub port: u16,
    pub busy_timeout_ms: u64,
    pub log_level: String,
    pub max_connections: u32,
    pub lock_timeout_secs: u64,
}

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("I/O error reading config: {0}")]
    Io(#[from] std::io::Error),
    #[error("TOML parse error: {0}")]
    Toml(#[from] toml::de::Error),
    #[error("Invalid value for '{field}': {value}")]
    InvalidValue { field: String, value: String },
}

/// Partial config loaded from a TOML file — all fields optional so defaults apply.
#[derive(Debug, Default, Deserialize)]
struct FileConfig {
    db_path: Option<PathBuf>,
    host: Option<String>,
    port: Option<u16>,
    busy_timeout_ms: Option<u64>,
    log_level: Option<String>,
    max_connections: Option<u32>,
    lock_timeout_secs: Option<u64>,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            db_path: default_db_path(),
            host: "127.0.0.1".to_string(),
            port: 7890,
            busy_timeout_ms: 5000,
            log_level: "info".to_string(),
            max_connections: 5,
            lock_timeout_secs: 30,
        }
    }
}

fn default_db_path() -> PathBuf {
    directories::ProjectDirs::from("dev", "oxidium", "forge")
        .map(|d| d.data_local_dir().join("forge.db"))
        .unwrap_or_else(|| PathBuf::from("forge.db"))
}

pub fn default_config_path() -> PathBuf {
    directories::ProjectDirs::from("dev", "oxidium", "forge")
        .map(|d| d.config_dir().join("oxidium-forge.toml"))
        .unwrap_or_else(|| PathBuf::from("config/oxidium-forge.toml"))
}

pub fn lock_file_path(config: &Config) -> PathBuf {
    let dir = config.db_path.parent()
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| PathBuf::from("."));
    dir.join(format!("forge-{}.lock", config.port))
}

impl Config {
    pub fn load() -> Result<Config, ConfigError> {
        let mut config = Config::default();

        // Load from config file
        let config_path = std::env::var("FORGE_CONFIG")
            .map(PathBuf::from)
            .unwrap_or_else(|_| default_config_path());

        if config_path.exists() {
            let toml_str = std::fs::read_to_string(&config_path)?;
            let file_cfg: FileConfig = toml::from_str(&toml_str)?;
            if let Some(v) = file_cfg.db_path { config.db_path = v; }
            if let Some(v) = file_cfg.host { config.host = v; }
            if let Some(v) = file_cfg.port { config.port = v; }
            if let Some(v) = file_cfg.busy_timeout_ms { config.busy_timeout_ms = v; }
            if let Some(v) = file_cfg.log_level { config.log_level = v; }
            if let Some(v) = file_cfg.max_connections { config.max_connections = v; }
            if let Some(v) = file_cfg.lock_timeout_secs { config.lock_timeout_secs = v; }
        }

        // Apply env vars (highest priority)
        if let Ok(v) = std::env::var("FORGE_DB_PATH") {
            config.db_path = PathBuf::from(v);
        }
        if let Ok(v) = std::env::var("FORGE_HOST") {
            config.host = v;
        }
        if let Ok(v) = std::env::var("FORGE_PORT") {
            config.port = v.parse().map_err(|_| ConfigError::InvalidValue {
                field: "FORGE_PORT".to_string(),
                value: v,
            })?;
        }
        if let Ok(v) = std::env::var("FORGE_LOG").or_else(|_| std::env::var("RUST_LOG")) {
            config.log_level = v;
        }
        if let Ok(v) = std::env::var("FORGE_MAX_CONNECTIONS") {
            config.max_connections = v.parse().map_err(|_| ConfigError::InvalidValue {
                field: "FORGE_MAX_CONNECTIONS".to_string(),
                value: v,
            })?;
        }

        config.validate()?;
        Ok(config)
    }

    fn validate(&self) -> Result<(), ConfigError> {
        if self.port == 0 {
            return Err(ConfigError::InvalidValue {
                field: "port".to_string(),
                value: "0".to_string(),
            });
        }
        if self.busy_timeout_ms == 0 {
            return Err(ConfigError::InvalidValue {
                field: "busy_timeout_ms".to_string(),
                value: "0".to_string(),
            });
        }
        if self.max_connections == 0 {
            return Err(ConfigError::InvalidValue {
                field: "max_connections".to_string(),
                value: "0".to_string(),
            });
        }
        Ok(())
    }

    pub fn base_url(&self) -> String {
        format!("http://{}:{}", self.host, self.port)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_valid() {
        let cfg = Config::default();
        assert!(cfg.validate().is_ok());
        assert_eq!(cfg.port, 7890);
        assert_eq!(cfg.host, "127.0.0.1");
    }

    #[test]
    fn env_override_wins() {
        std::env::set_var("FORGE_PORT", "9999");
        let cfg = Config::load().unwrap();
        assert_eq!(cfg.port, 9999);
        std::env::remove_var("FORGE_PORT");
    }

    #[test]
    fn invalid_port_zero_rejected() {
        let mut cfg = Config::default();
        cfg.port = 0;
        assert!(cfg.validate().is_err());
    }
}
