use forge_model::StoreError;
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct ProcessDefinitionRow {
    pub key: i64,
    pub bpmn_process_id: String,
    pub version: i64,
    pub resource_name: String,
    pub bpmn_xml: String,
    pub runtime_graph: String,
    pub deployed_at: String,
    pub is_active: i64,
    pub checksum: String,
}

pub struct DefinitionRepo<'a> {
    pub pool: &'a SqlitePool,
}

impl<'a> DefinitionRepo<'a> {
    pub fn new(pool: &'a SqlitePool) -> Self {
        DefinitionRepo { pool }
    }

    pub async fn insert_new_version(
        &self,
        bpmn_process_id: &str,
        resource_name: &str,
        bpmn_xml: &str,
        runtime_graph: &str,
        checksum: &str,
        auto_activate: bool,
    ) -> Result<ProcessDefinitionRow, StoreError> {
        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(|e| StoreError::Sqlx(e.to_string()))?;

        // Compute next version (max+1 per process id)
        let max_ver: Option<i64> =
            sqlx::query_scalar("SELECT MAX(version) FROM process_definitions WHERE bpmn_process_id = ?")
                .bind(bpmn_process_id)
                .fetch_one(&mut *tx)
                .await
                .map_err(|e| StoreError::Sqlx(e.to_string()))?;

        let next_version = max_ver.unwrap_or(0) + 1;

        // If auto_activate, deactivate the current active version first
        if auto_activate {
            sqlx::query(
                "UPDATE process_definitions SET is_active = 0 WHERE bpmn_process_id = ? AND is_active = 1",
            )
            .bind(bpmn_process_id)
            .execute(&mut *tx)
            .await
            .map_err(|e| StoreError::Sqlx(e.to_string()))?;
        }

        let is_active: i64 = if auto_activate { 1 } else { 0 };

        let key: i64 = sqlx::query_scalar(
            "INSERT INTO process_definitions (bpmn_process_id, version, resource_name, bpmn_xml, runtime_graph, checksum, is_active)
             VALUES (?, ?, ?, ?, ?, ?, ?)
             RETURNING key",
        )
        .bind(bpmn_process_id)
        .bind(next_version)
        .bind(resource_name)
        .bind(bpmn_xml)
        .bind(runtime_graph)
        .bind(checksum)
        .bind(is_active)
        .fetch_one(&mut *tx)
        .await
        .map_err(|e| StoreError::Sqlx(e.to_string()))?;

        tx.commit()
            .await
            .map_err(|e| StoreError::Sqlx(e.to_string()))?;

        self.get_by_key(key)
            .await?
            .ok_or_else(|| StoreError::Sqlx("Inserted row not found".to_string()))
    }

    pub async fn activate(&self, key: i64) -> Result<(), StoreError> {
        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(|e| StoreError::Sqlx(e.to_string()))?;

        // Get the process id for this key
        let process_id: Option<String> =
            sqlx::query_scalar("SELECT bpmn_process_id FROM process_definitions WHERE key = ?")
                .bind(key)
                .fetch_optional(&mut *tx)
                .await
                .map_err(|e| StoreError::Sqlx(e.to_string()))?;

        let process_id =
            process_id.ok_or_else(|| StoreError::Sqlx(format!("Definition {key} not found")))?;

        // Deactivate all others for this process id
        sqlx::query(
            "UPDATE process_definitions SET is_active = 0 WHERE bpmn_process_id = ? AND is_active = 1",
        )
        .bind(&process_id)
        .execute(&mut *tx)
        .await
        .map_err(|e| StoreError::Sqlx(e.to_string()))?;

        // Activate target
        sqlx::query("UPDATE process_definitions SET is_active = 1 WHERE key = ?")
            .bind(key)
            .execute(&mut *tx)
            .await
            .map_err(|e| StoreError::Sqlx(e.to_string()))?;

        tx.commit()
            .await
            .map_err(|e| StoreError::Sqlx(e.to_string()))
    }

    pub async fn get_active(
        &self,
        bpmn_process_id: &str,
    ) -> Result<Option<ProcessDefinitionRow>, StoreError> {
        sqlx::query_as::<_, ProcessDefinitionRow>(
            "SELECT * FROM process_definitions WHERE bpmn_process_id = ? AND is_active = 1",
        )
        .bind(bpmn_process_id)
        .fetch_optional(self.pool)
        .await
        .map_err(|e| StoreError::Sqlx(e.to_string()))
    }

    pub async fn get_by_key(&self, key: i64) -> Result<Option<ProcessDefinitionRow>, StoreError> {
        sqlx::query_as::<_, ProcessDefinitionRow>("SELECT * FROM process_definitions WHERE key = ?")
            .bind(key)
            .fetch_optional(self.pool)
            .await
            .map_err(|e| StoreError::Sqlx(e.to_string()))
    }

    pub async fn list(&self) -> Result<Vec<ProcessDefinitionRow>, StoreError> {
        sqlx::query_as::<_, ProcessDefinitionRow>(
            "SELECT * FROM process_definitions ORDER BY bpmn_process_id, version",
        )
        .fetch_all(self.pool)
        .await
        .map_err(|e| StoreError::Sqlx(e.to_string()))
    }

    pub async fn list_versions(
        &self,
        bpmn_process_id: &str,
    ) -> Result<Vec<ProcessDefinitionRow>, StoreError> {
        sqlx::query_as::<_, ProcessDefinitionRow>(
            "SELECT * FROM process_definitions WHERE bpmn_process_id = ? ORDER BY version",
        )
        .bind(bpmn_process_id)
        .fetch_all(self.pool)
        .await
        .map_err(|e| StoreError::Sqlx(e.to_string()))
    }
}
