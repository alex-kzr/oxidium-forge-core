use forge_model::StoreError;
use sqlx::SqlitePool;
use std::collections::BTreeMap;

/// All versioned migrations embedded at compile time.
/// Format: (version, name, sql)
const MIGRATIONS: &[(i64, &str, &str)] = &[
    (1, "system", include_str!("../../../migrations/0001_system.sql")),
    (2, "process_definitions", include_str!("../../../migrations/0002_process_definitions.sql")),
    (3, "runtime_core", include_str!("../../../migrations/0003_runtime_core.sql")),
    (4, "variables", include_str!("../../../migrations/0004_variables.sql")),
    (5, "process_events", include_str!("../../../migrations/0005_process_events.sql")),
    (6, "jobs_incidents", include_str!("../../../migrations/0006_jobs_incidents.sql")),
    (7, "manual_tasks", include_str!("../../../migrations/0007_manual_tasks.sql")),
];

pub async fn run_migrations(pool: &SqlitePool) -> Result<(), StoreError> {
    // Bootstrap: create schema_migrations if it doesn't exist yet.
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS schema_migrations (
            version    INTEGER PRIMARY KEY,
            name       TEXT NOT NULL,
            applied_at TEXT NOT NULL,
            checksum   TEXT NOT NULL
        )",
    )
    .execute(pool)
    .await
    .map_err(|e| StoreError::Migration(e.to_string()))?;

    // Load already-applied migrations.
    let applied: BTreeMap<i64, String> =
        sqlx::query_as::<_, (i64, String)>("SELECT version, checksum FROM schema_migrations")
            .fetch_all(pool)
            .await
            .map_err(|e| StoreError::Migration(e.to_string()))?
            .into_iter()
            .collect();

    for (version, name, sql) in MIGRATIONS {
        let checksum = compute_checksum(sql);

        if let Some(recorded_checksum) = applied.get(version) {
            if recorded_checksum != &checksum {
                return Err(StoreError::ChecksumMismatch { version: *version });
            }
            // Already applied and matches — skip.
            continue;
        }

        // Apply in its own transaction.
        let mut tx = pool
            .begin()
            .await
            .map_err(|e| StoreError::Migration(e.to_string()))?;
        sqlx::query(sql)
            .execute(&mut *tx)
            .await
            .map_err(|e| StoreError::Migration(format!("v{version} {name}: {e}")))?;
        sqlx::query(
            "INSERT INTO schema_migrations (version, name, applied_at, checksum)
             VALUES (?, ?, datetime('now'), ?)",
        )
        .bind(version)
        .bind(name)
        .bind(&checksum)
        .execute(&mut *tx)
        .await
        .map_err(|e| StoreError::Migration(e.to_string()))?;
        tx.commit()
            .await
            .map_err(|e| StoreError::Migration(e.to_string()))?;

        tracing::info!(version, name, "Applied migration");
    }

    Ok(())
}

pub fn migration_count() -> usize {
    MIGRATIONS.len()
}

fn compute_checksum(sql: &str) -> String {
    // Normalize line endings before hashing so Windows/Unix match.
    let normalized = sql.replace("\r\n", "\n");
    let digest = md5_hex(normalized.as_bytes());
    digest
}

fn md5_hex(data: &[u8]) -> String {
    // Simple FNV-1a based hash (no md5 dep needed) — deterministic, sufficient for drift detection.
    let mut hash: u64 = 14695981039346656037;
    for byte in data {
        hash ^= *byte as u64;
        hash = hash.wrapping_mul(1099511628211);
    }
    format!("{:016x}", hash)
}

#[cfg(test)]
mod tests {
    use super::*;
    use forge_model::Config;
    use crate::pool::Store;
    use tempfile::tempdir;

    #[tokio::test]
    async fn fresh_db_applies_all_migrations() {
        let dir = tempdir().unwrap();
        let mut config = Config::default();
        config.db_path = dir.path().join("test.db");

        let store = Store::connect(&config).await.unwrap();
        run_migrations(&store.pool).await.unwrap();

        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM schema_migrations")
            .fetch_one(&store.pool)
            .await
            .unwrap();
        assert_eq!(count, MIGRATIONS.len() as i64);
        assert_eq!(count, 7);

        store.close().await;
    }

    #[tokio::test]
    async fn second_run_is_noop() {
        let dir = tempdir().unwrap();
        let mut config = Config::default();
        config.db_path = dir.path().join("test2.db");

        let store = Store::connect(&config).await.unwrap();
        run_migrations(&store.pool).await.unwrap();
        run_migrations(&store.pool).await.unwrap(); // Should not error

        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM schema_migrations")
            .fetch_one(&store.pool)
            .await
            .unwrap();
        assert_eq!(count, MIGRATIONS.len() as i64);

        store.close().await;
    }

    #[test]
    fn checksum_stable() {
        let a = compute_checksum("SELECT 1;");
        let b = compute_checksum("SELECT 1;");
        assert_eq!(a, b);
    }

    #[test]
    fn checksum_differs() {
        let a = compute_checksum("SELECT 1;");
        let b = compute_checksum("SELECT 2;");
        assert_ne!(a, b);
    }
}
