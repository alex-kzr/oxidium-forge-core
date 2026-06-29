use crate::error::EngineError;
use forge_store::{events, incidents};
use sqlx::SqliteConnection;

/// Create an incident record and append the audit event — must be inside a transaction.
pub async fn create_incident(
    conn: &mut SqliteConnection,
    instance_key: i64,
    definition_key: i64,
    element_id: Option<&str>,
    element_instance_key: Option<i64>,
    job_key: Option<i64>,
    incident_type: &str,
    message: &str,
) -> Result<i64, EngineError> {
    let row = incidents::insert_incident(
        conn,
        instance_key,
        element_instance_key,
        job_key,
        incident_type,
        message,
    )
    .await?;

    events::append_event(
        conn,
        Some(instance_key),
        Some(definition_key),
        element_id,
        "incident.created",
        &serde_json::json!({
            "incident_key": row.key,
            "incident_type": incident_type,
            "message": message,
        }),
    )
    .await?;

    Ok(row.key)
}
