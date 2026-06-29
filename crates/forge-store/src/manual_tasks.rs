use forge_model::StoreError;
use serde::{Deserialize, Serialize};
use sqlx::{SqliteConnection, SqlitePool};

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct ManualTaskRow {
    pub key: i64,
    pub instance_key: i64,
    pub element_instance_key: i64,
    pub element_id: String,
    pub state: String,
    pub variables: String,
    pub created_at: String,
    pub completed_at: Option<String>,
}

pub async fn insert_manual_task(
    conn: &mut SqliteConnection,
    instance_key: i64,
    element_instance_key: i64,
    element_id: &str,
    variables_json: &str,
) -> Result<ManualTaskRow, StoreError> {
    let key: i64 = sqlx::query_scalar(
        "INSERT INTO manual_tasks (instance_key, element_instance_key, element_id, variables)
         VALUES (?, ?, ?, ?) RETURNING key",
    )
    .bind(instance_key)
    .bind(element_instance_key)
    .bind(element_id)
    .bind(variables_json)
    .fetch_one(&mut *conn)
    .await
    .map_err(|e| StoreError::Sqlx(e.to_string()))?;

    get_manual_task_conn(conn, key)
        .await?
        .ok_or_else(|| StoreError::Sqlx("ManualTask not found after insert".into()))
}

pub async fn get_manual_task_conn(
    conn: &mut SqliteConnection,
    key: i64,
) -> Result<Option<ManualTaskRow>, StoreError> {
    sqlx::query_as::<_, ManualTaskRow>("SELECT * FROM manual_tasks WHERE key = ?")
        .bind(key)
        .fetch_optional(conn)
        .await
        .map_err(|e| StoreError::Sqlx(e.to_string()))
}

pub async fn get_manual_task(
    pool: &SqlitePool,
    key: i64,
) -> Result<Option<ManualTaskRow>, StoreError> {
    sqlx::query_as::<_, ManualTaskRow>("SELECT * FROM manual_tasks WHERE key = ?")
        .bind(key)
        .fetch_optional(pool)
        .await
        .map_err(|e| StoreError::Sqlx(e.to_string()))
}

pub async fn complete_manual_task(conn: &mut SqliteConnection, key: i64) -> Result<(), StoreError> {
    sqlx::query(
        "UPDATE manual_tasks
         SET state = 'completed', completed_at = datetime('now')
         WHERE key = ?",
    )
    .bind(key)
    .execute(conn)
    .await
    .map_err(|e| StoreError::Sqlx(e.to_string()))?;
    Ok(())
}

pub async fn cancel_manual_task(conn: &mut SqliteConnection, key: i64) -> Result<(), StoreError> {
    sqlx::query(
        "UPDATE manual_tasks
         SET state = 'cancelled', completed_at = datetime('now')
         WHERE key = ?",
    )
    .bind(key)
    .execute(conn)
    .await
    .map_err(|e| StoreError::Sqlx(e.to_string()))?;
    Ok(())
}

pub async fn list_manual_tasks(
    pool: &SqlitePool,
    state_filter: Option<&str>,
    instance_key_filter: Option<i64>,
) -> Result<Vec<ManualTaskRow>, StoreError> {
    match (state_filter, instance_key_filter) {
        (Some(state), Some(instance)) => {
            sqlx::query_as::<_, ManualTaskRow>(
                "SELECT * FROM manual_tasks WHERE state = ? AND instance_key = ? ORDER BY created_at",
            )
            .bind(state)
            .bind(instance)
            .fetch_all(pool)
            .await
            .map_err(|e| StoreError::Sqlx(e.to_string()))
        }
        (Some(state), None) => {
            sqlx::query_as::<_, ManualTaskRow>(
                "SELECT * FROM manual_tasks WHERE state = ? ORDER BY created_at",
            )
            .bind(state)
            .fetch_all(pool)
            .await
            .map_err(|e| StoreError::Sqlx(e.to_string()))
        }
        (None, Some(instance)) => {
            sqlx::query_as::<_, ManualTaskRow>(
                "SELECT * FROM manual_tasks WHERE instance_key = ? ORDER BY created_at",
            )
            .bind(instance)
            .fetch_all(pool)
            .await
            .map_err(|e| StoreError::Sqlx(e.to_string()))
        }
        (None, None) => {
            sqlx::query_as::<_, ManualTaskRow>(
                "SELECT * FROM manual_tasks ORDER BY created_at",
            )
            .fetch_all(pool)
            .await
            .map_err(|e| StoreError::Sqlx(e.to_string()))
        }
    }
}

pub async fn list_manual_tasks_for_instance(
    pool: &SqlitePool,
    instance_key: i64,
) -> Result<Vec<ManualTaskRow>, StoreError> {
    sqlx::query_as::<_, ManualTaskRow>(
        "SELECT * FROM manual_tasks WHERE instance_key = ? ORDER BY created_at",
    )
    .bind(instance_key)
    .fetch_all(pool)
    .await
    .map_err(|e| StoreError::Sqlx(e.to_string()))
}

pub async fn archive_manual_tasks_for_instance(
    conn: &mut SqliteConnection,
    instance_key: i64,
) -> Result<(), StoreError> {
    sqlx::query(
        "INSERT INTO manual_tasks_history
         (key, instance_key, element_instance_key, element_id, state, variables, created_at, completed_at)
         SELECT key, instance_key, element_instance_key, element_id, state, variables, created_at, completed_at
         FROM manual_tasks WHERE instance_key = ?",
    )
    .bind(instance_key)
    .execute(&mut *conn)
    .await
    .map_err(|e| StoreError::Sqlx(e.to_string()))?;

    sqlx::query("DELETE FROM manual_tasks WHERE instance_key = ?")
        .bind(instance_key)
        .execute(&mut *conn)
        .await
        .map_err(|e| StoreError::Sqlx(e.to_string()))?;
    Ok(())
}
