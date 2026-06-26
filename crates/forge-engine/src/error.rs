use forge_model::StoreError;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum EngineError {
    #[error("Store error: {0}")]
    Store(#[from] StoreError),
    #[error("No active definition for process: {0}")]
    NoActiveDefinition(String),
    #[error("Invalid runtime graph: {0}")]
    InvalidGraph(String),
    #[error("Expression error: {0}")]
    Expression(String),
    #[error("Step limit exceeded ({0} steps)")]
    MaxSteps(usize),
    #[error("Serialization error: {0}")]
    Json(#[from] serde_json::Error),
}
