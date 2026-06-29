use crate::engine::step::{StepContext, StepOutcome};
use crate::error::EngineError;
use crate::mapping::apply_inputs;
use forge_bpmn::graph::{CompiledNode, NodeKind};
use forge_store::{events, jobs, runtime, variables};

pub async fn handle_service_task(
    ctx: &mut StepContext<'_>,
    node: &CompiledNode,
) -> Result<StepOutcome, EngineError> {
    let (task_type, retries, io_mapping) = match &node.kind {
        NodeKind::ServiceTask {
            task_type,
            retries,
            io_mapping,
        } => (task_type.clone(), *retries, io_mapping.clone()),
        _ => unreachable!(),
    };

    // Defensive check: task_type must not be empty (should be caught at compile time).
    if task_type.is_empty() {
        let msg = format!("ServiceTask '{}': missing task type", node.id);
        events::append_event(
            ctx.conn,
            Some(ctx.instance_key),
            Some(ctx.definition_key),
            Some(&node.id),
            "incident.created",
            &serde_json::json!({"reason": msg}),
        )
        .await?;
        return Ok(StepOutcome::Incident(msg));
    }

    // Insert element instance (remains active while job is pending).
    let ei = runtime::insert_element_instance(
        ctx.conn,
        ctx.instance_key,
        ctx.execution_key,
        &node.id,
        "serviceTask",
    )
    .await?;

    events::append_event(
        ctx.conn,
        Some(ctx.instance_key),
        Some(ctx.definition_key),
        Some(&node.id),
        "element.entered",
        &serde_json::json!({"element_type": "serviceTask", "task_type": task_type}),
    )
    .await?;

    // Evaluate input mapping to build job payload.
    let scope_vars = variables::get_all_for_eval(ctx.conn, ctx.instance_key).await?;
    let job_vars = apply_inputs(&scope_vars, io_mapping.as_ref())?;
    let vars_json = serde_json::to_string(&job_vars)
        .map_err(|e| EngineError::Json(e))?;

    // Create the job.
    let job = jobs::insert_job(
        ctx.conn,
        ctx.instance_key,
        ei.key,
        &node.id,
        &task_type,
        retries as i64,
        &vars_json,
    )
    .await?;

    events::append_event(
        ctx.conn,
        Some(ctx.instance_key),
        Some(ctx.definition_key),
        Some(&node.id),
        "job.created",
        &serde_json::json!({
            "job_key": job.key,
            "task_type": task_type,
            "retries": retries
        }),
    )
    .await?;

    Ok(StepOutcome::Wait)
}
