use crate::error::EngineError;
use forge_store::{history, runtime};
use sqlx::SqliteConnection;

/// Mark the instance terminal and move all its rows into history.
/// Must be called inside the step transaction.
pub async fn archive_instance(
    conn: &mut SqliteConnection,
    instance_key: i64,
    status: &str,
) -> Result<(), EngineError> {
    runtime::update_instance_status(conn, instance_key, status).await?;
    history::archive_instance(conn, instance_key).await?;
    Ok(())
}
