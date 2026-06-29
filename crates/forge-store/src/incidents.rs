use forge_model::StoreError;
use serde::{Deserialize, Serialize};
use sqlx::{SqliteConnection, SqlitePool};

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct IncidentRow {
    pub key: i64,
    pub instance_key: i64,
    pub element_instance_key: Option<i64>,
    pub job_key: Option<i64>,
    pub incident_type: String,
    pub message: String,
    pub state: String,
    pub created_at: String,
    pub resolved_at: Option<String>,
}

pub async fn insert_incident(
    conn: &mut SqliteConnection,
    instance_key: i64,
    element_instance_key: Option<i64>,
    job_key: Option<i64>,
    incident_type: &str,
    message: &str,
) -> Result<IncidentRow, StoreError> {
    let key: i64 = sqlx::query_scalar(
        "INSERT INTO incidents (instance_key, element_instance_key, job_key, incident_type, message)
         VALUES (?, ?, ?, ?, ?) RETURNING key",
    )
    .bind(instance_key)
    .bind(element_instance_key)
    .bind(job_key)
    .bind(incident_type)
    .bind(message)
    .fetch_one(&mut *conn)
    .await
    .map_err(|e| StoreError::Sqlx(e.to_string()))?;

    get_incident_conn(conn, key)
        .await?
        .ok_or_else(|| StoreError::Sqlx("Incident not found after insert".into()))
}

pub async fn get_incident_conn(
    conn: &mut SqliteConnection,
    key: i64,
) -> Result<Option<IncidentRow>, StoreError> {
    sqlx::query_as::<_, IncidentRow>("SELECT * FROM incidents WHERE key = ?")
        .bind(key)
        .fetch_optional(conn)
        .await
        .map_err(|e| StoreError::Sqlx(e.to_string()))
}

pub async fn get_incident(pool: &SqlitePool, key: i64) -> Result<Option<IncidentRow>, StoreError> {
    sqlx::query_as::<_, IncidentRow>("SELECT * FROM incidents WHERE key = ?")
        .bind(key)
        .fetch_optional(pool)
        .await
        .map_err(|e| StoreError::Sqlx(e.to_string()))
}

pub async fn list_incidents(
    pool: &SqlitePool,
    state_filter: Option<&str>,
) -> Result<Vec<IncidentRow>, StoreError> {
    match state_filter {
        Some(s) => sqlx::query_as::<_, IncidentRow>(
            "SELECT * FROM incidents WHERE state = ? ORDER BY created_at DESC",
        )
        .bind(s)
        .fetch_all(pool)
        .await
        .map_err(|e| StoreError::Sqlx(e.to_string())),
        None => sqlx::query_as::<_, IncidentRow>(
            "SELECT * FROM incidents ORDER BY created_at DESC",
        )
        .fetch_all(pool)
        .await
        .map_err(|e| StoreError::Sqlx(e.to_string())),
    }
}

pub async fn resolve_incident_conn(
    conn: &mut SqliteConnection,
    key: i64,
) -> Result<(), StoreError> {
    sqlx::query(
        "UPDATE incidents
         SET state = 'resolved', resolved_at = datetime('now')
         WHERE key = ?",
    )
    .bind(key)
    .execute(conn)
    .await
    .map_err(|e| StoreError::Sqlx(e.to_string()))?;
    Ok(())
}

pub async fn archive_incidents_for_instance(
    conn: &mut SqliteConnection,
    instance_key: i64,
) -> Result<(), StoreError> {
    sqlx::query(
        "INSERT INTO incidents_history
         (key, instance_key, element_instance_key, job_key, incident_type, message,
          state, created_at, resolved_at)
         SELECT key, instance_key, element_instance_key, job_key, incident_type, message,
                state, created_at, resolved_at
         FROM incidents WHERE instance_key = ?",
    )
    .bind(instance_key)
    .execute(&mut *conn)
    .await
    .map_err(|e| StoreError::Sqlx(e.to_string()))?;

    sqlx::query("DELETE FROM incidents WHERE instance_key = ?")
        .bind(instance_key)
        .execute(&mut *conn)
        .await
        .map_err(|e| StoreError::Sqlx(e.to_string()))?;
    Ok(())
}
