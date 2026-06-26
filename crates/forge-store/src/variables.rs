use forge_model::StoreError;
use serde_json::Value;
use sqlx::{SqliteConnection, SqlitePool};
use std::collections::HashMap;

pub async fn set_variable(
    conn: &mut SqliteConnection,
    instance_key: i64,
    scope_key: i64,
    name: &str,
    value: &Value,
) -> Result<(), StoreError> {
    let json = serde_json::to_string(value).map_err(|e| StoreError::Sqlx(e.to_string()))?;
    sqlx::query(
        "INSERT INTO variables (instance_key, scope_key, name, value)
         VALUES (?, ?, ?, ?)
         ON CONFLICT(instance_key, scope_key, name)
         DO UPDATE SET value = excluded.value, updated_at = datetime('now')",
    )
    .bind(instance_key)
    .bind(scope_key)
    .bind(name)
    .bind(&json)
    .execute(conn)
    .await
    .map_err(|e| StoreError::Sqlx(e.to_string()))?;
    Ok(())
}

/// Load all variables for an instance as a flat map (used by the expression evaluator).
/// When multiple scopes define the same name, the first row (ordered by scope_key ASC) wins,
/// which means the most-local scope (lowest numeric key — the root instance key — comes first).
/// For the MVP single-execution model all vars share the same scope_key so there is no conflict.
pub async fn get_all_for_eval(
    conn: &mut SqliteConnection,
    instance_key: i64,
) -> Result<HashMap<String, Value>, StoreError> {
    let rows: Vec<(String, String)> = sqlx::query_as(
        "SELECT name, value FROM variables WHERE instance_key = ? ORDER BY scope_key",
    )
    .bind(instance_key)
    .fetch_all(conn)
    .await
    .map_err(|e| StoreError::Sqlx(e.to_string()))?;

    let mut map = HashMap::new();
    for (name, val_str) in rows {
        let val: Value =
            serde_json::from_str(&val_str).map_err(|e| StoreError::Sqlx(e.to_string()))?;
        map.entry(name).or_insert(val);
    }
    Ok(map)
}

/// Read all variables for an instance for the REST API.
pub async fn get_all_for_instance(
    pool: &SqlitePool,
    instance_key: i64,
) -> Result<HashMap<String, Value>, StoreError> {
    let rows: Vec<(String, String)> = sqlx::query_as(
        "SELECT name, value FROM variables WHERE instance_key = ? ORDER BY scope_key",
    )
    .bind(instance_key)
    .fetch_all(pool)
    .await
    .map_err(|e| StoreError::Sqlx(e.to_string()))?;

    let mut map = HashMap::new();
    for (name, val_str) in rows {
        let val: Value =
            serde_json::from_str(&val_str).map_err(|e| StoreError::Sqlx(e.to_string()))?;
        map.entry(name).or_insert(val);
    }
    Ok(map)
}
