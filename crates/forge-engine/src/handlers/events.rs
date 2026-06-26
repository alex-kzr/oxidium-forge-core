use crate::engine::archive::archive_instance;
use crate::engine::step::{StepContext, StepOutcome};
use crate::error::EngineError;
use forge_bpmn::graph::{CompiledNode, NodeKind};
use forge_store::{events, runtime};

pub async fn handle_start_event(
    ctx: &mut StepContext<'_>,
    node: &CompiledNode,
) -> Result<StepOutcome, EngineError> {
    let ei = runtime::insert_element_instance(
        ctx.conn,
        ctx.instance_key,
        ctx.execution_key,
        &node.id,
        "startEvent",
    )
    .await?;

    events::append_event(
        ctx.conn,
        Some(ctx.instance_key),
        Some(ctx.definition_key),
        Some(&node.id),
        "element.entered",
        &serde_json::json!({"element_type": "startEvent"}),
    )
    .await?;

    let outgoing = ctx
        .graph
        .outgoing
        .get(&node.id)
        .ok_or_else(|| {
            EngineError::InvalidGraph(format!(
                "StartEvent '{}' has no outgoing flows",
                node.id
            ))
        })?;

    if outgoing.is_empty() {
        return Err(EngineError::InvalidGraph(format!(
            "StartEvent '{}' has no outgoing flows",
            node.id
        )));
    }

    // Start events always have exactly one outgoing flow (validated by forge-bpmn).
    let next_node_id = outgoing[0].target.clone();

    runtime::complete_element_instance(ctx.conn, ei.key).await?;

    events::append_event(
        ctx.conn,
        Some(ctx.instance_key),
        Some(ctx.definition_key),
        Some(&node.id),
        "element.left",
        &serde_json::json!({"element_type": "startEvent", "next": next_node_id}),
    )
    .await?;

    Ok(StepOutcome::Continue(next_node_id))
}

pub async fn handle_end_event(
    ctx: &mut StepContext<'_>,
    node: &CompiledNode,
) -> Result<StepOutcome, EngineError> {
    // Verify node kind matches.
    debug_assert!(matches!(node.kind, NodeKind::EndEvent));

    let ei = runtime::insert_element_instance(
        ctx.conn,
        ctx.instance_key,
        ctx.execution_key,
        &node.id,
        "endEvent",
    )
    .await?;

    events::append_event(
        ctx.conn,
        Some(ctx.instance_key),
        Some(ctx.definition_key),
        Some(&node.id),
        "element.entered",
        &serde_json::json!({"element_type": "endEvent"}),
    )
    .await?;

    runtime::complete_element_instance(ctx.conn, ei.key).await?;

    events::append_event(
        ctx.conn,
        Some(ctx.instance_key),
        Some(ctx.definition_key),
        Some(&node.id),
        "element.left",
        &serde_json::json!({"element_type": "endEvent"}),
    )
    .await?;

    // Complete this execution.
    runtime::complete_execution(ctx.conn, ctx.execution_key).await?;

    // Check if any other executions are still active.
    let active = runtime::count_active_executions(ctx.conn, ctx.instance_key).await?;
    if active == 0 {
        events::append_event(
            ctx.conn,
            Some(ctx.instance_key),
            Some(ctx.definition_key),
            None,
            "instance.completed",
            &serde_json::json!({}),
        )
        .await?;

        archive_instance(ctx.conn, ctx.instance_key, "completed").await?;
    }

    Ok(StepOutcome::End)
}
