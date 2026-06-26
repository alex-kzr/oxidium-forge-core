use forge_model::StoreError;
use serde_json::Value;
use sqlx::{SqliteConnection, SqlitePool};

/// Append an audit event within an active transaction.
pub async fn append_event(
    conn: &mut SqliteConnection,
    instance_key: Option<i64>,
    definition_key: Option<i64>,
    element_id: Option<&str>,
    event_type: &str,
    payload: &Value,
) -> Result<(), StoreError> {
    let payload_str =
        serde_json::to_string(payload).map_err(|e| StoreError::Sqlx(e.to_string()))?;
    sqlx::query(
        "INSERT INTO process_events
         (instance_key, definition_key, element_id, event_type, payload)
         VALUES (?, ?, ?, ?, ?)",
    )
    .bind(instance_key)
    .bind(definition_key)
    .bind(element_id)
    .bind(event_type)
    .bind(&payload_str)
    .execute(conn)
    .await
    .map_err(|e| StoreError::Sqlx(e.to_string()))?;
    Ok(())
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, sqlx::FromRow)]
pub struct ProcessEventRow {
    pub key: i64,
    pub instance_key: Option<i64>,
    pub definition_key: Option<i64>,
    pub element_id: Option<String>,
    pub event_type: String,
    pub payload: String,
    pub created_at: String,
}

pub async fn list_events_for_instance(
    pool: &SqlitePool,
    instance_key: i64,
) -> Result<Vec<ProcessEventRow>, StoreError> {
    sqlx::query_as::<_, ProcessEventRow>(
        "SELECT * FROM process_events WHERE instance_key = ? ORDER BY created_at, key",
    )
    .bind(instance_key)
    .fetch_all(pool)
    .await
    .map_err(|e| StoreError::Sqlx(e.to_string()))
}
