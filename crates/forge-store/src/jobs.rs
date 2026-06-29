use forge_model::StoreError;
use serde::{Deserialize, Serialize};
use sqlx::{SqliteConnection, SqlitePool};

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct JobRow {
    pub key: i64,
    pub instance_key: i64,
    pub element_instance_key: i64,
    pub element_id: String,
    pub task_type: String,
    pub state: String,
    pub retries: i64,
    pub worker: Option<String>,
    pub locked_until: Option<String>,
    pub retry_at: Option<String>,
    pub variables: String,
    pub error_message: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

pub async fn insert_job(
    conn: &mut SqliteConnection,
    instance_key: i64,
    element_instance_key: i64,
    element_id: &str,
    task_type: &str,
    retries: i64,
    variables_json: &str,
) -> Result<JobRow, StoreError> {
    let key: i64 = sqlx::query_scalar(
        "INSERT INTO jobs (instance_key, element_instance_key, element_id, task_type, retries, variables)
         VALUES (?, ?, ?, ?, ?, ?) RETURNING key",
    )
    .bind(instance_key)
    .bind(element_instance_key)
    .bind(element_id)
    .bind(task_type)
    .bind(retries)
    .bind(variables_json)
    .fetch_one(&mut *conn)
    .await
    .map_err(|e| StoreError::Sqlx(e.to_string()))?;

    get_job_conn(conn, key)
        .await?
        .ok_or_else(|| StoreError::Sqlx("Job not found after insert".into()))
}

pub async fn get_job_conn(
    conn: &mut SqliteConnection,
    key: i64,
) -> Result<Option<JobRow>, StoreError> {
    sqlx::query_as::<_, JobRow>("SELECT * FROM jobs WHERE key = ?")
        .bind(key)
        .fetch_optional(conn)
        .await
        .map_err(|e| StoreError::Sqlx(e.to_string()))
}

pub async fn get_job(pool: &SqlitePool, key: i64) -> Result<Option<JobRow>, StoreError> {
    sqlx::query_as::<_, JobRow>("SELECT * FROM jobs WHERE key = ?")
        .bind(key)
        .fetch_optional(pool)
        .await
        .map_err(|e| StoreError::Sqlx(e.to_string()))
}

/// Atomically select up to `max_jobs` activatable jobs of `task_type` and lock them.
/// Also reclaims activated jobs whose `locked_until` has expired.
pub async fn activate_jobs(
    conn: &mut SqliteConnection,
    task_type: &str,
    worker: &str,
    max_jobs: i64,
    lock_duration_secs: i64,
) -> Result<Vec<JobRow>, StoreError> {
    // Find candidate keys in one query.
    let keys: Vec<i64> = sqlx::query_scalar(
        "SELECT key FROM jobs
         WHERE task_type = ?
           AND (
               (state = 'activatable' AND (retry_at IS NULL OR retry_at <= datetime('now')))
               OR (state = 'activated' AND locked_until <= datetime('now'))
           )
         ORDER BY key
         LIMIT ?",
    )
    .bind(task_type)
    .bind(max_jobs)
    .fetch_all(&mut *conn)
    .await
    .map_err(|e| StoreError::Sqlx(e.to_string()))?;

    if keys.is_empty() {
        return Ok(vec![]);
    }

    // Update each one and collect results.
    let mut result = Vec::with_capacity(keys.len());
    for key in keys {
        sqlx::query(
            "UPDATE jobs
             SET state = 'activated',
                 worker = ?,
                 locked_until = datetime('now', ? || ' seconds'),
                 updated_at = datetime('now')
             WHERE key = ?",
        )
        .bind(worker)
        .bind(lock_duration_secs.to_string())
        .bind(key)
        .execute(&mut *conn)
        .await
        .map_err(|e| StoreError::Sqlx(e.to_string()))?;

        if let Some(row) = get_job_conn(conn, key).await? {
            result.push(row);
        }
    }

    Ok(result)
}

pub async fn complete_job(
    conn: &mut SqliteConnection,
    key: i64,
) -> Result<(), StoreError> {
    sqlx::query(
        "UPDATE jobs SET state = 'completed', updated_at = datetime('now') WHERE key = ?",
    )
    .bind(key)
    .execute(conn)
    .await
    .map_err(|e| StoreError::Sqlx(e.to_string()))?;
    Ok(())
}

pub async fn fail_job(
    conn: &mut SqliteConnection,
    key: i64,
    retries: i64,
    error_message: &str,
    retry_backoff_secs: Option<i64>,
) -> Result<(), StoreError> {
    let retry_at = retry_backoff_secs.map(|s| format!("+{s} seconds"));
    sqlx::query(
        "UPDATE jobs
         SET state = 'activatable',
             retries = ?,
             error_message = ?,
             worker = NULL,
             locked_until = NULL,
             retry_at = CASE WHEN ? IS NOT NULL THEN datetime('now', ?) ELSE NULL END,
             updated_at = datetime('now')
         WHERE key = ?",
    )
    .bind(retries)
    .bind(error_message)
    .bind(retry_at.as_deref())
    .bind(retry_at.as_deref())
    .bind(key)
    .execute(conn)
    .await
    .map_err(|e| StoreError::Sqlx(e.to_string()))?;
    Ok(())
}

pub async fn mark_job_dead(
    conn: &mut SqliteConnection,
    key: i64,
    error_message: &str,
) -> Result<(), StoreError> {
    sqlx::query(
        "UPDATE jobs
         SET state = 'dead', error_message = ?, worker = NULL, locked_until = NULL,
             updated_at = datetime('now')
         WHERE key = ?",
    )
    .bind(error_message)
    .bind(key)
    .execute(conn)
    .await
    .map_err(|e| StoreError::Sqlx(e.to_string()))?;
    Ok(())
}

pub async fn reset_dead_job(
    conn: &mut SqliteConnection,
    key: i64,
    retries: i64,
) -> Result<(), StoreError> {
    sqlx::query(
        "UPDATE jobs
         SET state = 'activatable', retries = ?, worker = NULL, locked_until = NULL,
             retry_at = NULL, error_message = NULL, updated_at = datetime('now')
         WHERE key = ?",
    )
    .bind(retries)
    .bind(key)
    .execute(conn)
    .await
    .map_err(|e| StoreError::Sqlx(e.to_string()))?;
    Ok(())
}

pub async fn list_jobs_for_instance(
    pool: &SqlitePool,
    instance_key: i64,
) -> Result<Vec<JobRow>, StoreError> {
    sqlx::query_as::<_, JobRow>(
        "SELECT * FROM jobs WHERE instance_key = ? ORDER BY created_at",
    )
    .bind(instance_key)
    .fetch_all(pool)
    .await
    .map_err(|e| StoreError::Sqlx(e.to_string()))
}

pub async fn archive_jobs_for_instance(
    conn: &mut SqliteConnection,
    instance_key: i64,
) -> Result<(), StoreError> {
    sqlx::query(
        "INSERT INTO jobs_history
         (key, instance_key, element_instance_key, element_id, task_type, state, retries,
          worker, locked_until, retry_at, variables, error_message, created_at, updated_at)
         SELECT key, instance_key, element_instance_key, element_id, task_type, state, retries,
                worker, locked_until, retry_at, variables, error_message, created_at, updated_at
         FROM jobs WHERE instance_key = ?",
    )
    .bind(instance_key)
    .execute(&mut *conn)
    .await
    .map_err(|e| StoreError::Sqlx(e.to_string()))?;

    sqlx::query("DELETE FROM jobs WHERE instance_key = ?")
        .bind(instance_key)
        .execute(&mut *conn)
        .await
        .map_err(|e| StoreError::Sqlx(e.to_string()))?;
    Ok(())
}
