mod events;
mod gateway;

use crate::engine::step::{StepContext, StepOutcome};
use crate::error::EngineError;
use forge_bpmn::graph::{CompiledNode, NodeKind};

pub async fn dispatch(
    ctx: &mut StepContext<'_>,
    node: &CompiledNode,
) -> Result<StepOutcome, EngineError> {
    match &node.kind {
        NodeKind::StartEvent => events::handle_start_event(ctx, node).await,
        NodeKind::EndEvent => events::handle_end_event(ctx, node).await,
        NodeKind::ExclusiveGateway { .. } => gateway::handle_exclusive_gateway(ctx, node).await,
        // Wait states implemented in later phases — park the execution.
        NodeKind::ServiceTask { .. } | NodeKind::ManualTask { .. } => Ok(StepOutcome::Wait),
    }
}
