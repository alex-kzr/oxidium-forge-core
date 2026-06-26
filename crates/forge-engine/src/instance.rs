use crate::engine::run::run_step;
use crate::error::EngineError;
use forge_model::StoreError;
use forge_store::{definitions::DefinitionRepo, events, runtime, variables, Store};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StartedInstance {
    pub instance_key: i64,
    pub execution_key: i64,
    pub status: String,
    pub bpmn_process_id: String,
    pub version: i64,
}

/// Start a new process instance from the active definition version.
/// Accepts optional initial variables (key → JSON value).
/// Returns after stepping to the first wait state or terminal.
pub async fn start_instance(
    store: &Store,
    bpmn_process_id: &str,
    initial_vars: HashMap<String, Value>,
) -> Result<StartedInstance, EngineError> {
    // 1. Resolve the active definition.
    let repo = DefinitionRepo::new(&store.pool);
    let def = repo
        .get_active(bpmn_process_id)
        .await?
        .ok_or_else(|| EngineError::NoActiveDefinition(bpmn_process_id.to_string()))?;

    // 2. Deserialize the runtime graph.
    let graph: forge_bpmn::graph::RuntimeGraph = serde_json::from_str(&def.runtime_graph)?;

    // 3. Create instance + root execution + initial variables inside a transaction.
    let (instance_key, execution_key) = {
        let mut tx = store
            .pool
            .begin()
            .await
            .map_err(|e| EngineError::Store(StoreError::Sqlx(e.to_string())))?;

        let instance =
            runtime::insert_instance(&mut *tx, def.key, bpmn_process_id, def.version).await?;

        // Store initial variables at root scope (scope_key = instance_key).
        for (name, value) in &initial_vars {
            variables::set_variable(&mut *tx, instance.key, instance.key, name, value).await?;
        }

        let execution =
            runtime::insert_execution(&mut *tx, instance.key, None, &graph.start_node).await?;

        events::append_event(
            &mut *tx,
            Some(instance.key),
            Some(def.key),
            None,
            "instance.started",
            &serde_json::json!({
                "bpmn_process_id": bpmn_process_id,
                "version": def.version,
                "definition_key": def.key,
            }),
        )
        .await?;

        tx.commit()
            .await
            .map_err(|e| EngineError::Store(StoreError::Sqlx(e.to_string())))?;

        (instance.key, execution.key)
    };

    // 4. Run the first step (advances from start node to first wait state or terminal).
    run_step(
        &store.pool,
        instance_key,
        execution_key,
        def.key,
        &graph,
    )
    .await?;

    // 5. Read the final status (may be in runtime or history).
    let status = resolve_instance_status(&store.pool, instance_key).await?;

    Ok(StartedInstance {
        instance_key,
        execution_key,
        status,
        bpmn_process_id: bpmn_process_id.to_string(),
        version: def.version,
    })
}

/// Read instance status from runtime, falling back to history for terminal instances.
pub async fn resolve_instance_status(
    pool: &sqlx::SqlitePool,
    instance_key: i64,
) -> Result<String, EngineError> {
    if let Some(row) = runtime::get_instance(pool, instance_key).await? {
        return Ok(row.status);
    }
    if let Some(hist) = runtime::get_instance_history(pool, instance_key).await? {
        return Ok(hist.status);
    }
    Err(EngineError::Store(StoreError::Sqlx(format!(
        "Instance {instance_key} not found"
    ))))
}
