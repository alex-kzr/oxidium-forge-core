use crate::error::EngineError;
use crate::handlers;
use forge_bpmn::graph::RuntimeGraph;
use sqlx::SqliteConnection;

/// Outcome returned by each element handler.
#[derive(Debug, Clone)]
pub enum StepOutcome {
    /// Token moves to the specified node id; the loop continues.
    Continue(String),
    /// Execution is parked at a wait state (job, manual task, message, timer).
    Wait,
    /// The process instance reached a terminal end event and has been archived.
    End,
    /// An unrecoverable condition was raised; the engine recorded an incident.
    Incident(String),
}

/// Everything a handler needs during one atomic step.
pub struct StepContext<'a> {
    pub conn: &'a mut SqliteConnection,
    pub graph: &'a RuntimeGraph,
    pub instance_key: i64,
    pub execution_key: i64,
    pub definition_key: i64,
}

const MAX_STEPS: usize = 1000;

/// Advance the execution from `start_node_id` until a wait state, end, or incident.
/// All DB writes happen through `ctx.conn` (which is already an open transaction).
pub async fn step_execution(
    ctx: &mut StepContext<'_>,
    start_node_id: &str,
) -> Result<StepOutcome, EngineError> {
    let mut current_node_id = start_node_id.to_string();

    for _ in 0..MAX_STEPS {
        let node = ctx
            .graph
            .nodes
            .get(&current_node_id)
            .ok_or_else(|| {
                EngineError::InvalidGraph(format!(
                    "Node '{}' not found in runtime graph",
                    current_node_id
                ))
            })?
            .clone();

        let outcome = handlers::dispatch(ctx, &node).await?;

        match outcome {
            StepOutcome::Continue(next_id) => {
                forge_store::runtime::update_execution_node(
                    ctx.conn,
                    ctx.execution_key,
                    &next_id,
                )
                .await?;
                current_node_id = next_id;
            }
            other => return Ok(other),
        }
    }

    Err(EngineError::MaxSteps(MAX_STEPS))
}
