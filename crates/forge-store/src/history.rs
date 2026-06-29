use crate::{incidents, jobs};
use forge_model::StoreError;
use sqlx::SqliteConnection;

/// Move a completed/failed/cancelled instance and all its child rows into the history tables.
/// Must be called inside the step transaction immediately after the terminal status is written.
pub async fn archive_instance(
    conn: &mut SqliteConnection,
    instance_key: i64,
) -> Result<(), StoreError> {
    // Element instances → history
    sqlx::query(
        "INSERT INTO process_element_instances_history
         (key, instance_key, execution_key, element_id, element_type, state, entered_at, left_at)
         SELECT key, instance_key, execution_key, element_id, element_type, state,
                entered_at, COALESCE(left_at, datetime('now'))
         FROM process_element_instances
         WHERE instance_key = ?",
    )
    .bind(instance_key)
    .execute(&mut *conn)
    .await
    .map_err(|e| StoreError::Sqlx(e.to_string()))?;

    sqlx::query("DELETE FROM process_element_instances WHERE instance_key = ?")
        .bind(instance_key)
        .execute(&mut *conn)
        .await
        .map_err(|e| StoreError::Sqlx(e.to_string()))?;

    // Executions → history
    sqlx::query(
        "INSERT INTO executions_history
         (key, instance_key, parent_execution_key, current_node_id, state, created_at, ended_at)
         SELECT key, instance_key, parent_execution_key, current_node_id, state,
                created_at, datetime('now')
         FROM executions
         WHERE instance_key = ?",
    )
    .bind(instance_key)
    .execute(&mut *conn)
    .await
    .map_err(|e| StoreError::Sqlx(e.to_string()))?;

    sqlx::query("DELETE FROM executions WHERE instance_key = ?")
        .bind(instance_key)
        .execute(&mut *conn)
        .await
        .map_err(|e| StoreError::Sqlx(e.to_string()))?;

    // Variables → history
    sqlx::query(
        "INSERT INTO variables_history
         (key, instance_key, scope_key, name, value, updated_at)
         SELECT key, instance_key, scope_key, name, value, updated_at
         FROM variables
         WHERE instance_key = ?",
    )
    .bind(instance_key)
    .execute(&mut *conn)
    .await
    .map_err(|e| StoreError::Sqlx(e.to_string()))?;

    sqlx::query("DELETE FROM variables WHERE instance_key = ?")
        .bind(instance_key)
        .execute(&mut *conn)
        .await
        .map_err(|e| StoreError::Sqlx(e.to_string()))?;

    // Jobs → history
    jobs::archive_jobs_for_instance(conn, instance_key).await?;

    // Incidents → history
    incidents::archive_incidents_for_instance(conn, instance_key).await?;

    // Instance → history (last, after FKs are resolved by deleting children first)
    sqlx::query(
        "INSERT INTO process_instances_history
         (key, definition_key, bpmn_process_id, version, status, started_at,
          ended_at, parent_instance_key)
         SELECT key, definition_key, bpmn_process_id, version, status, started_at,
                COALESCE(ended_at, datetime('now')), parent_instance_key
         FROM process_instances
         WHERE key = ?",
    )
    .bind(instance_key)
    .execute(&mut *conn)
    .await
    .map_err(|e| StoreError::Sqlx(e.to_string()))?;

    sqlx::query("DELETE FROM process_instances WHERE key = ?")
        .bind(instance_key)
        .execute(&mut *conn)
        .await
        .map_err(|e| StoreError::Sqlx(e.to_string()))?;

    Ok(())
}
