use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use forge_engine::jobs as engine_jobs;
use forge_store::incidents;
use serde::Deserialize;

use crate::state::AppState;

#[derive(Debug, Deserialize)]
pub struct ListIncidentsQuery {
    pub state: Option<String>,
}

pub async fn list_incidents(
    State(state): State<AppState>,
    Query(params): Query<ListIncidentsQuery>,
) -> impl IntoResponse {
    match incidents::list_incidents(&state.store.pool, params.state.as_deref()).await {
        Ok(rows) => {
            let data: Vec<_> = rows
                .into_iter()
                .map(|r| {
                    serde_json::json!({
                        "key": r.key,
                        "instance_key": r.instance_key,
                        "element_instance_key": r.element_instance_key,
                        "job_key": r.job_key,
                        "incident_type": r.incident_type,
                        "message": r.message,
                        "state": r.state,
                        "created_at": r.created_at,
                        "resolved_at": r.resolved_at,
                    })
                })
                .collect();
            Json(serde_json::json!({"incidents": data})).into_response()
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

pub async fn resolve_incident(
    State(state): State<AppState>,
    Path(key): Path<i64>,
) -> impl IntoResponse {
    match engine_jobs::resolve_incident(&state.store, key).await {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(forge_engine::EngineError::Store(forge_model::StoreError::Sqlx(msg)))
            if msg.contains("not found") =>
        {
            (
                StatusCode::NOT_FOUND,
                Json(serde_json::json!({"error": msg})),
            )
                .into_response()
        }
        Err(forge_engine::EngineError::Store(forge_model::StoreError::Sqlx(msg)))
            if msg.contains("not active") =>
        {
            (
                StatusCode::CONFLICT,
                Json(serde_json::json!({"error": msg})),
            )
                .into_response()
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}
