use crate::engine::step::{step_execution, StepContext, StepOutcome};
use crate::error::EngineError;
use forge_bpmn::graph::RuntimeGraph;
use forge_model::StoreError;
use forge_store::runtime;
use sqlx::SqlitePool;

/// Open a transaction, advance the execution to the next wait state, and commit.
/// On error, the transaction is rolled back automatically when dropped.
pub async fn run_step(
    pool: &SqlitePool,
    instance_key: i64,
    execution_key: i64,
    definition_key: i64,
    graph: &RuntimeGraph,
) -> Result<StepOutcome, EngineError> {
    let mut tx = pool
        .begin()
        .await
        .map_err(|e| EngineError::Store(StoreError::Sqlx(e.to_string())))?;

    let exec = runtime::get_execution_conn(&mut *tx, execution_key)
        .await?
        .ok_or_else(|| {
            EngineError::Store(StoreError::Sqlx(format!(
                "Execution {execution_key} not found"
            )))
        })?;

    let start_node = exec.current_node_id.clone();

    let outcome = {
        let mut ctx = StepContext {
            conn: &mut *tx,
            graph,
            instance_key,
            execution_key,
            definition_key,
        };
        step_execution(&mut ctx, &start_node).await?
    };

    tx.commit()
        .await
        .map_err(|e| EngineError::Store(StoreError::Sqlx(e.to_string())))?;

    Ok(outcome)
}
