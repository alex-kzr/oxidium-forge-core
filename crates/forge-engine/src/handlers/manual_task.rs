use crate::engine::step::{StepContext, StepOutcome};
use crate::error::EngineError;
use crate::mapping::apply_inputs;
use forge_bpmn::graph::{CompiledNode, NodeKind};
use forge_store::{events, manual_tasks, runtime, variables};

pub async fn handle_manual_task(
    ctx: &mut StepContext<'_>,
    node: &CompiledNode,
) -> Result<StepOutcome, EngineError> {
    let io_mapping = match &node.kind {
        NodeKind::ManualTask { io_mapping } => io_mapping.clone(),
        _ => unreachable!(),
    };

    let ei = runtime::insert_element_instance(
        ctx.conn,
        ctx.instance_key,
        ctx.execution_key,
        &node.id,
        "manualTask",
    )
    .await?;

    events::append_event(
        ctx.conn,
        Some(ctx.instance_key),
        Some(ctx.definition_key),
        Some(&node.id),
        "element.entered",
        &serde_json::json!({"element_type": "manualTask"}),
    )
    .await?;

    let scope_vars = variables::get_all_for_eval(ctx.conn, ctx.instance_key).await?;
    let task_vars = apply_inputs(&scope_vars, io_mapping.as_ref())?;
    let vars_json =
        serde_json::to_string(&task_vars).map_err(EngineError::Json)?;

    let task = manual_tasks::insert_manual_task(
        ctx.conn,
        ctx.instance_key,
        ei.key,
        &node.id,
        &vars_json,
    )
    .await?;

    events::append_event(
        ctx.conn,
        Some(ctx.instance_key),
        Some(ctx.definition_key),
        Some(&node.id),
        "manual_task.created",
        &serde_json::json!({"manual_task_key": task.key}),
    )
    .await?;

    Ok(StepOutcome::Wait)
}
