use crate::engine::step::{StepContext, StepOutcome};
use crate::error::EngineError;
use crate::expr::{eval, parse};
use forge_bpmn::graph::{CompiledNode, NodeKind};
use forge_store::{events, runtime, variables};

pub async fn handle_exclusive_gateway(
    ctx: &mut StepContext<'_>,
    node: &CompiledNode,
) -> Result<StepOutcome, EngineError> {
    debug_assert!(matches!(node.kind, NodeKind::ExclusiveGateway { .. }));

    let ei = runtime::insert_element_instance(
        ctx.conn,
        ctx.instance_key,
        ctx.execution_key,
        &node.id,
        "exclusiveGateway",
    )
    .await?;

    events::append_event(
        ctx.conn,
        Some(ctx.instance_key),
        Some(ctx.definition_key),
        Some(&node.id),
        "element.entered",
        &serde_json::json!({"element_type": "exclusiveGateway"}),
    )
    .await?;

    // Load variables for expression evaluation.
    let vars = variables::get_all_for_eval(ctx.conn, ctx.instance_key).await?;

    let outgoing = ctx
        .graph
        .outgoing
        .get(&node.id)
        .map(|v| v.as_slice())
        .unwrap_or(&[]);

    // Try each non-default flow's condition in order.
    let mut chosen: Option<String> = None;
    let mut default_target: Option<String> = None;

    for flow in outgoing {
        if flow.is_default {
            default_target = Some(flow.target.clone());
            continue;
        }

        match &flow.condition {
            None => {
                // Unconditional non-default flow on a gateway — take it immediately.
                chosen = Some(flow.target.clone());
                break;
            }
            Some(cond_str) => {
                let expr = match parse(cond_str) {
                    Ok(e) => e,
                    Err(e) => {
                        // Parse failure → incident (not an engine-level bug).
                        let reason = format!(
                            "ExclusiveGateway '{}': flow '{}' condition parse error: {}",
                            node.id, flow.id, e
                        );
                        runtime::complete_element_instance(ctx.conn, ei.key).await?;
                        events::append_event(
                            ctx.conn,
                            Some(ctx.instance_key),
                            Some(ctx.definition_key),
                            Some(&node.id),
                            "incident.created",
                            &serde_json::json!({"reason": reason}),
                        )
                        .await?;
                        return Ok(StepOutcome::Incident(reason));
                    }
                };
                let result = match eval(&expr, &vars) {
                    Ok(v) => v,
                    Err(e) => {
                        // Eval failure (e.g., unknown variable) → incident.
                        let reason = format!(
                            "ExclusiveGateway '{}': flow '{}' condition eval error: {}",
                            node.id, flow.id, e
                        );
                        runtime::complete_element_instance(ctx.conn, ei.key).await?;
                        events::append_event(
                            ctx.conn,
                            Some(ctx.instance_key),
                            Some(ctx.definition_key),
                            Some(&node.id),
                            "incident.created",
                            &serde_json::json!({"reason": reason}),
                        )
                        .await?;
                        return Ok(StepOutcome::Incident(reason));
                    }
                };
                if result.as_bool().unwrap_or(false) {
                    chosen = Some(flow.target.clone());
                    break;
                }
            }
        }
    }

    let target = chosen.or(default_target);

    runtime::complete_element_instance(ctx.conn, ei.key).await?;

    match target {
        Some(next_id) => {
            events::append_event(
                ctx.conn,
                Some(ctx.instance_key),
                Some(ctx.definition_key),
                Some(&node.id),
                "element.left",
                &serde_json::json!({
                    "element_type": "exclusiveGateway",
                    "next": next_id
                }),
            )
            .await?;

            Ok(StepOutcome::Continue(next_id))
        }
        None => {
            let reason = format!(
                "ExclusiveGateway '{}': no outgoing flow condition matched and no default flow",
                node.id
            );
            events::append_event(
                ctx.conn,
                Some(ctx.instance_key),
                Some(ctx.definition_key),
                Some(&node.id),
                "incident.created",
                &serde_json::json!({"reason": reason}),
            )
            .await?;

            Ok(StepOutcome::Incident(reason))
        }
    }
}
