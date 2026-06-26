use axum::{extract::State, http::StatusCode, response::IntoResponse, Json};
use forge_store::migrate::migration_count;
use serde_json::json;

use crate::state::AppState;

pub async fn handler(State(state): State<AppState>) -> impl IntoResponse {
    let db_ok = sqlx::query("SELECT 1")
        .fetch_one(&state.store.pool)
        .await
        .is_ok();

    let applied = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM schema_migrations",
    )
    .fetch_one(&state.store.pool)
    .await
    .unwrap_or(0);

    let expected = migration_count() as i64;
    let migrations_ok = db_ok && applied >= expected;

    let body = json!({
        "status": if db_ok && migrations_ok { "ok" } else { "degraded" },
        "db": if db_ok { "connected" } else { "unreachable" },
        "migrations": { "applied": applied, "expected": expected },
        "version": env!("CARGO_PKG_VERSION"),
    });

    if db_ok && migrations_ok {
        (StatusCode::OK, Json(body)).into_response()
    } else {
        (StatusCode::SERVICE_UNAVAILABLE, Json(body)).into_response()
    }
}
