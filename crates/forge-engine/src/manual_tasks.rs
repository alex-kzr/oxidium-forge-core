use crate::engine::{archive::archive_instance, run::run_step};
use crate::error::EngineError;
use crate::mapping::apply_outputs;
use forge_bpmn::graph::{NodeKind, RuntimeGraph};
use forge_model::StoreError;
use forge_store::{definitions::DefinitionRepo, events, manual_tasks, runtime, variables, Store};
use serde_json::Value;
use std::collections::HashMap;

/// Complete an open manual task, apply output mapping, and resume stepping.
/// Returns the new instance status.
pub async fn complete_manual_task(
    store: &Store,
    task_key: i64,
    output_vars: HashMap<String, Value>,
) -> Result<String, EngineError> {
    let task = manual_tasks::get_manual_task(&store.pool, task_key)
        .await?
        .ok_or_else(|| {
            EngineError::Store(StoreError::Sqlx(format!("ManualTask {task_key} not found")))
        })?;

    if task.state != "open" {
        return Err(EngineError::Store(StoreError::Sqlx(format!(
            "ManualTask {task_key} is not open (state={})",
            task.state
        ))));
    }

    let instance = runtime::get_instance(&store.pool, task.instance_key)
        .await?
        .ok_or_else(|| {
            EngineError::Store(StoreError::Sqlx(format!(
                "Instance {} not found",
                task.instance_key
            )))
        })?;

    let repo = DefinitionRepo::new(&store.pool);
    let def = repo
        .get_by_key(instance.definition_key)
        .await?
        .ok_or_else(|| {
            EngineError::Store(StoreError::Sqlx(format!(
                "Definition {} not found",
                instance.definition_key
            )))
        })?;

    let graph: RuntimeGraph = serde_json::from_str(&def.runtime_graph)?;

    let next_node_id = graph
        .outgoing
        .get(&task.element_id)
        .and_then(|flows| flows.first())
        .map(|f| f.target.clone())
        .ok_or_else(|| {
            EngineError::InvalidGraph(format!(
                "ManualTask '{}' has no outgoing flow",
                task.element_id
            ))
        })?;

    let execution = find_execution_at(store, task.instance_key, &task.element_id).await?;

    {
        let mut tx = store
            .pool
            .begin()
            .await
            .map_err(|e| EngineError::Store(StoreError::Sqlx(e.to_string())))?;

        let mut scope_vars = variables::get_all_for_eval(&mut *tx, task.instance_key).await?;
        let io_mapping = graph.nodes.get(&task.element_id).and_then(|n| match &n.kind {
            NodeKind::ManualTask { io_mapping, .. } => io_mapping.as_ref(),
            _ => None,
        });
        apply_outputs(&mut scope_vars, &output_vars, io_mapping)?;
        for (name, val) in &scope_vars {
            variables::set_variable(&mut *tx, task.instance_key, task.instance_key, name, val)
                .await?;
        }

        manual_tasks::complete_manual_task(&mut *tx, task_key).await?;
        runtime::complete_element_instance(&mut *tx, task.element_instance_key).await?;
        runtime::update_execution_node(&mut *tx, execution.key, &next_node_id).await?;

        events::append_event(
            &mut *tx,
            Some(task.instance_key),
            Some(def.key),
            Some(&task.element_id),
            "manual_task.completed",
            &serde_json::json!({"manual_task_key": task_key, "next": next_node_id}),
        )
        .await?;

        tx.commit()
            .await
            .map_err(|e| EngineError::Store(StoreError::Sqlx(e.to_string())))?;
    }

    run_step(
        &store.pool,
        task.instance_key,
        execution.key,
        def.key,
        &graph,
    )
    .await?;

    crate::instance::resolve_instance_status(&store.pool, task.instance_key).await
}

/// Cancel an open manual task and move the instance to cancelled + history.
pub async fn cancel_manual_task(
    store: &Store,
    task_key: i64,
    reason: &str,
) -> Result<(), EngineError> {
    let task = manual_tasks::get_manual_task(&store.pool, task_key)
        .await?
        .ok_or_else(|| {
            EngineError::Store(StoreError::Sqlx(format!("ManualTask {task_key} not found")))
        })?;

    if task.state != "open" {
        return Err(EngineError::Store(StoreError::Sqlx(format!(
            "ManualTask {task_key} is not open (state={})",
            task.state
        ))));
    }

    let instance = runtime::get_instance(&store.pool, task.instance_key)
        .await?
        .ok_or_else(|| {
            EngineError::Store(StoreError::Sqlx(format!(
                "Instance {} not found",
                task.instance_key
            )))
        })?;

    let mut tx = store
        .pool
        .begin()
        .await
        .map_err(|e| EngineError::Store(StoreError::Sqlx(e.to_string())))?;

    manual_tasks::cancel_manual_task(&mut *tx, task_key).await?;

    events::append_event(
        &mut *tx,
        Some(task.instance_key),
        Some(instance.definition_key),
        Some(&task.element_id),
        "manual_task.cancelled",
        &serde_json::json!({"manual_task_key": task_key, "reason": reason}),
    )
    .await?;

    events::append_event(
        &mut *tx,
        Some(task.instance_key),
        Some(instance.definition_key),
        None,
        "instance.cancelled",
        &serde_json::json!({"reason": reason}),
    )
    .await?;

    // Archive with status = 'cancelled'. archive_instance sets status then moves all rows.
    archive_instance(&mut *tx, task.instance_key, "cancelled").await?;

    tx.commit()
        .await
        .map_err(|e| EngineError::Store(StoreError::Sqlx(e.to_string())))?;

    Ok(())
}

// ---------------------------------------------------------------------------

async fn find_execution_at(
    store: &Store,
    instance_key: i64,
    element_id: &str,
) -> Result<runtime::ExecutionRow, EngineError> {
    let row: Option<runtime::ExecutionRow> = sqlx::query_as(
        "SELECT * FROM executions
         WHERE instance_key = ? AND current_node_id = ? AND state = 'active'",
    )
    .bind(instance_key)
    .bind(element_id)
    .fetch_optional(&store.pool)
    .await
    .map_err(|e| EngineError::Store(StoreError::Sqlx(e.to_string())))?;

    row.ok_or_else(|| {
        EngineError::Store(StoreError::Sqlx(format!(
            "No active execution for instance {instance_key} at '{element_id}'"
        )))
    })
}
