use forge_model::StoreError;
use serde::{Deserialize, Serialize};
use sqlx::{SqliteConnection, SqlitePool};

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct ProcessInstanceRow {
    pub key: i64,
    pub definition_key: i64,
    pub bpmn_process_id: String,
    pub version: i64,
    pub status: String,
    pub started_at: String,
    pub ended_at: Option<String>,
    pub parent_instance_key: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct ProcessInstanceHistoryRow {
    pub key: i64,
    pub definition_key: i64,
    pub bpmn_process_id: String,
    pub version: i64,
    pub status: String,
    pub started_at: String,
    pub ended_at: String,
    pub parent_instance_key: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct ExecutionRow {
    pub key: i64,
    pub instance_key: i64,
    pub parent_execution_key: Option<i64>,
    pub current_node_id: String,
    pub state: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct ElementInstanceRow {
    pub key: i64,
    pub instance_key: i64,
    pub execution_key: i64,
    pub element_id: String,
    pub element_type: String,
    pub state: String,
    pub entered_at: String,
    pub left_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct ElementInstanceHistoryRow {
    pub key: i64,
    pub instance_key: i64,
    pub execution_key: i64,
    pub element_id: String,
    pub element_type: String,
    pub state: String,
    pub entered_at: String,
    pub left_at: String,
}

// --- Write operations (used inside transactions via &mut *tx) ---

pub async fn insert_instance(
    conn: &mut SqliteConnection,
    definition_key: i64,
    bpmn_process_id: &str,
    version: i64,
) -> Result<ProcessInstanceRow, StoreError> {
    let key: i64 = sqlx::query_scalar(
        "INSERT INTO process_instances (definition_key, bpmn_process_id, version)
         VALUES (?, ?, ?) RETURNING key",
    )
    .bind(definition_key)
    .bind(bpmn_process_id)
    .bind(version)
    .fetch_one(&mut *conn)
    .await
    .map_err(|e| StoreError::Sqlx(e.to_string()))?;

    get_instance_conn(conn, key)
        .await?
        .ok_or_else(|| StoreError::Sqlx("Instance not found after insert".into()))
}

pub async fn get_instance_conn(
    conn: &mut SqliteConnection,
    key: i64,
) -> Result<Option<ProcessInstanceRow>, StoreError> {
    sqlx::query_as::<_, ProcessInstanceRow>(
        "SELECT * FROM process_instances WHERE key = ?",
    )
    .bind(key)
    .fetch_optional(conn)
    .await
    .map_err(|e| StoreError::Sqlx(e.to_string()))
}

pub async fn update_instance_status(
    conn: &mut SqliteConnection,
    key: i64,
    status: &str,
) -> Result<(), StoreError> {
    let terminal = matches!(status, "completed" | "failed" | "cancelled");
    if terminal {
        sqlx::query(
            "UPDATE process_instances SET status = ?, ended_at = datetime('now') WHERE key = ?",
        )
        .bind(status)
        .bind(key)
        .execute(conn)
        .await
        .map_err(|e| StoreError::Sqlx(e.to_string()))?;
    } else {
        sqlx::query("UPDATE process_instances SET status = ? WHERE key = ?")
            .bind(status)
            .bind(key)
            .execute(conn)
            .await
            .map_err(|e| StoreError::Sqlx(e.to_string()))?;
    }
    Ok(())
}

pub async fn insert_execution(
    conn: &mut SqliteConnection,
    instance_key: i64,
    parent_execution_key: Option<i64>,
    current_node_id: &str,
) -> Result<ExecutionRow, StoreError> {
    let key: i64 = sqlx::query_scalar(
        "INSERT INTO executions (instance_key, parent_execution_key, current_node_id)
         VALUES (?, ?, ?) RETURNING key",
    )
    .bind(instance_key)
    .bind(parent_execution_key)
    .bind(current_node_id)
    .fetch_one(&mut *conn)
    .await
    .map_err(|e| StoreError::Sqlx(e.to_string()))?;

    get_execution_conn(conn, key)
        .await?
        .ok_or_else(|| StoreError::Sqlx("Execution not found after insert".into()))
}

pub async fn get_execution_conn(
    conn: &mut SqliteConnection,
    key: i64,
) -> Result<Option<ExecutionRow>, StoreError> {
    sqlx::query_as::<_, ExecutionRow>("SELECT * FROM executions WHERE key = ?")
        .bind(key)
        .fetch_optional(conn)
        .await
        .map_err(|e| StoreError::Sqlx(e.to_string()))
}

pub async fn update_execution_node(
    conn: &mut SqliteConnection,
    key: i64,
    current_node_id: &str,
) -> Result<(), StoreError> {
    sqlx::query("UPDATE executions SET current_node_id = ? WHERE key = ?")
        .bind(current_node_id)
        .bind(key)
        .execute(conn)
        .await
        .map_err(|e| StoreError::Sqlx(e.to_string()))?;
    Ok(())
}

pub async fn complete_execution(
    conn: &mut SqliteConnection,
    key: i64,
) -> Result<(), StoreError> {
    sqlx::query("UPDATE executions SET state = 'completed' WHERE key = ?")
        .bind(key)
        .execute(conn)
        .await
        .map_err(|e| StoreError::Sqlx(e.to_string()))?;
    Ok(())
}

pub async fn count_active_executions(
    conn: &mut SqliteConnection,
    instance_key: i64,
) -> Result<i64, StoreError> {
    sqlx::query_scalar(
        "SELECT COUNT(*) FROM executions WHERE instance_key = ? AND state = 'active'",
    )
    .bind(instance_key)
    .fetch_one(conn)
    .await
    .map_err(|e| StoreError::Sqlx(e.to_string()))
}

pub async fn insert_element_instance(
    conn: &mut SqliteConnection,
    instance_key: i64,
    execution_key: i64,
    element_id: &str,
    element_type: &str,
) -> Result<ElementInstanceRow, StoreError> {
    let key: i64 = sqlx::query_scalar(
        "INSERT INTO process_element_instances
         (instance_key, execution_key, element_id, element_type)
         VALUES (?, ?, ?, ?) RETURNING key",
    )
    .bind(instance_key)
    .bind(execution_key)
    .bind(element_id)
    .bind(element_type)
    .fetch_one(&mut *conn)
    .await
    .map_err(|e| StoreError::Sqlx(e.to_string()))?;

    get_element_instance_conn(conn, key)
        .await?
        .ok_or_else(|| StoreError::Sqlx("Element instance not found after insert".into()))
}

pub async fn get_element_instance_conn(
    conn: &mut SqliteConnection,
    key: i64,
) -> Result<Option<ElementInstanceRow>, StoreError> {
    sqlx::query_as::<_, ElementInstanceRow>(
        "SELECT * FROM process_element_instances WHERE key = ?",
    )
    .bind(key)
    .fetch_optional(conn)
    .await
    .map_err(|e| StoreError::Sqlx(e.to_string()))
}

pub async fn complete_element_instance(
    conn: &mut SqliteConnection,
    key: i64,
) -> Result<(), StoreError> {
    sqlx::query(
        "UPDATE process_element_instances
         SET state = 'completed', left_at = datetime('now') WHERE key = ?",
    )
    .bind(key)
    .execute(conn)
    .await
    .map_err(|e| StoreError::Sqlx(e.to_string()))?;
    Ok(())
}

// --- Read operations (used by the REST API) ---

pub async fn get_instance(
    pool: &SqlitePool,
    key: i64,
) -> Result<Option<ProcessInstanceRow>, StoreError> {
    sqlx::query_as::<_, ProcessInstanceRow>(
        "SELECT * FROM process_instances WHERE key = ?",
    )
    .bind(key)
    .fetch_optional(pool)
    .await
    .map_err(|e| StoreError::Sqlx(e.to_string()))
}

pub async fn get_instance_history(
    pool: &SqlitePool,
    key: i64,
) -> Result<Option<ProcessInstanceHistoryRow>, StoreError> {
    sqlx::query_as::<_, ProcessInstanceHistoryRow>(
        "SELECT * FROM process_instances_history WHERE key = ?",
    )
    .bind(key)
    .fetch_optional(pool)
    .await
    .map_err(|e| StoreError::Sqlx(e.to_string()))
}

pub async fn list_active_element_instances(
    pool: &SqlitePool,
    instance_key: i64,
) -> Result<Vec<ElementInstanceRow>, StoreError> {
    sqlx::query_as::<_, ElementInstanceRow>(
        "SELECT * FROM process_element_instances WHERE instance_key = ? ORDER BY entered_at",
    )
    .bind(instance_key)
    .fetch_all(pool)
    .await
    .map_err(|e| StoreError::Sqlx(e.to_string()))
}

pub async fn list_element_instances_history(
    pool: &SqlitePool,
    instance_key: i64,
) -> Result<Vec<ElementInstanceHistoryRow>, StoreError> {
    sqlx::query_as::<_, ElementInstanceHistoryRow>(
        "SELECT * FROM process_element_instances_history WHERE instance_key = ? ORDER BY entered_at",
    )
    .bind(instance_key)
    .fetch_all(pool)
    .await
    .map_err(|e| StoreError::Sqlx(e.to_string()))
}
