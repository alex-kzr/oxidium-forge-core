use crate::engine::run::run_step;
use crate::error::EngineError;
use crate::incidents::create_incident;
use crate::mapping::apply_outputs;
use forge_bpmn::graph::{NodeKind, RuntimeGraph};
use forge_model::StoreError;
use forge_store::{definitions::DefinitionRepo, events, jobs, runtime, variables, Store};
use serde_json::Value;
use std::collections::HashMap;

/// Complete an activated job, apply output mapping, then resume stepping.
/// Returns the new instance status.
pub async fn complete_job(
    store: &Store,
    job_key: i64,
    output_vars: HashMap<String, Value>,
) -> Result<String, EngineError> {
    // 1. Load job and validate state.
    let job = jobs::get_job(&store.pool, job_key)
        .await?
        .ok_or_else(|| EngineError::Store(StoreError::Sqlx(format!("Job {job_key} not found"))))?;

    if job.state != "activated" {
        return Err(EngineError::Store(StoreError::Sqlx(format!(
            "Job {job_key} is not activated (state={})",
            job.state
        ))));
    }

    // 2. Load the instance and its runtime graph.
    let instance = runtime::get_instance(&store.pool, job.instance_key)
        .await?
        .ok_or_else(|| {
            EngineError::Store(StoreError::Sqlx(format!(
                "Instance {} not found",
                job.instance_key
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

    // 3. Resolve the next node from the service task's outgoing flow.
    let next_node_id = graph
        .outgoing
        .get(&job.element_id)
        .and_then(|flows| flows.first())
        .map(|f| f.target.clone())
        .ok_or_else(|| {
            EngineError::InvalidGraph(format!(
                "ServiceTask '{}' has no outgoing flow",
                job.element_id
            ))
        })?;

    // 4. Find the execution parked at this service task.
    let execution = find_execution_at(store, job.instance_key, &job.element_id).await?;

    // 5. Apply outputs, complete job + element instance, advance execution — all in one tx.
    {
        let mut tx = store
            .pool
            .begin()
            .await
            .map_err(|e| EngineError::Store(StoreError::Sqlx(e.to_string())))?;

        // Evaluate and write output variables.
        let mut scope_vars = variables::get_all_for_eval(&mut *tx, job.instance_key).await?;
        let io_mapping = graph.nodes.get(&job.element_id).and_then(|n| match &n.kind {
            NodeKind::ServiceTask { io_mapping, .. } => io_mapping.as_ref(),
            _ => None,
        });
        apply_outputs(&mut scope_vars, &output_vars, io_mapping)?;
        for (name, val) in &scope_vars {
            variables::set_variable(&mut *tx, job.instance_key, job.instance_key, name, val)
                .await?;
        }

        // Mark job completed.
        jobs::complete_job(&mut *tx, job_key).await?;

        // Complete the element instance.
        runtime::complete_element_instance(&mut *tx, job.element_instance_key).await?;

        // Advance the execution past the service task so the next run_step starts from next_node.
        runtime::update_execution_node(&mut *tx, execution.key, &next_node_id).await?;

        events::append_event(
            &mut *tx,
            Some(job.instance_key),
            Some(def.key),
            Some(&job.element_id),
            "job.completed",
            &serde_json::json!({"job_key": job_key, "next": next_node_id}),
        )
        .await?;

        tx.commit()
            .await
            .map_err(|e| EngineError::Store(StoreError::Sqlx(e.to_string())))?;
    }

    // 6. Resume stepping from the next node.
    run_step(
        &store.pool,
        job.instance_key,
        execution.key,
        def.key,
        &graph,
    )
    .await?;

    crate::instance::resolve_instance_status(&store.pool, job.instance_key).await
}

/// Fail a job. If retries > 0 → returns to activatable (with optional backoff).
/// If retries == 0 → marks dead and raises an incident.
pub async fn fail_job(
    store: &Store,
    job_key: i64,
    error_message: &str,
    retries: i64,
    retry_backoff_secs: Option<i64>,
) -> Result<(), EngineError> {
    let job = jobs::get_job(&store.pool, job_key)
        .await?
        .ok_or_else(|| EngineError::Store(StoreError::Sqlx(format!("Job {job_key} not found"))))?;

    if job.state != "activated" {
        return Err(EngineError::Store(StoreError::Sqlx(format!(
            "Job {job_key} is not activated (state={})",
            job.state
        ))));
    }

    let instance = runtime::get_instance(&store.pool, job.instance_key)
        .await?
        .ok_or_else(|| {
            EngineError::Store(StoreError::Sqlx(format!(
                "Instance {} not found",
                job.instance_key
            )))
        })?;

    let mut tx = store
        .pool
        .begin()
        .await
        .map_err(|e| EngineError::Store(StoreError::Sqlx(e.to_string())))?;

    if retries > 0 {
        jobs::fail_job(&mut *tx, job_key, retries, error_message, retry_backoff_secs).await?;
        events::append_event(
            &mut *tx,
            Some(job.instance_key),
            Some(instance.definition_key),
            Some(&job.element_id),
            "job.failed",
            &serde_json::json!({
                "job_key": job_key,
                "error_message": error_message,
                "retries_remaining": retries,
            }),
        )
        .await?;
    } else {
        jobs::mark_job_dead(&mut *tx, job_key, error_message).await?;
        events::append_event(
            &mut *tx,
            Some(job.instance_key),
            Some(instance.definition_key),
            Some(&job.element_id),
            "job.failed",
            &serde_json::json!({
                "job_key": job_key,
                "error_message": error_message,
                "retries_remaining": 0,
            }),
        )
        .await?;
        create_incident(
            &mut *tx,
            job.instance_key,
            instance.definition_key,
            Some(job.element_id.as_str()),
            Some(job.element_instance_key),
            Some(job_key),
            "retry-exhausted",
            error_message,
        )
        .await?;
    }

    tx.commit()
        .await
        .map_err(|e| EngineError::Store(StoreError::Sqlx(e.to_string())))?;

    Ok(())
}

/// Resolve an incident: mark resolved, apply kind-specific remediation, emit audit.
pub async fn resolve_incident(store: &Store, incident_key: i64) -> Result<(), EngineError> {
    use forge_store::incidents;

    let incident = incidents::get_incident(&store.pool, incident_key)
        .await?
        .ok_or_else(|| {
            EngineError::Store(StoreError::Sqlx(format!(
                "Incident {incident_key} not found"
            )))
        })?;

    if incident.state != "active" {
        return Err(EngineError::Store(StoreError::Sqlx(format!(
            "Incident {incident_key} is not active"
        ))));
    }

    let definition_key = runtime::get_instance(&store.pool, incident.instance_key)
        .await?
        .map(|i| i.definition_key)
        .unwrap_or(0);

    let mut tx = store
        .pool
        .begin()
        .await
        .map_err(|e| EngineError::Store(StoreError::Sqlx(e.to_string())))?;

    incidents::resolve_incident_conn(&mut *tx, incident_key).await?;

    if incident.incident_type == "retry-exhausted" {
        if let Some(job_key) = incident.job_key {
            jobs::reset_dead_job(&mut *tx, job_key, 3).await?;
        }
    }

    events::append_event(
        &mut *tx,
        Some(incident.instance_key),
        Some(definition_key),
        None,
        "incident.resolved",
        &serde_json::json!({
            "incident_key": incident_key,
            "incident_type": incident.incident_type,
        }),
    )
    .await?;

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
