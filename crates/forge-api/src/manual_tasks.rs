use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use forge_engine::manual_tasks as engine_mt;
use forge_store::manual_tasks;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;

use crate::state::AppState;

// GET /api/v1/manual-tasks?state=open&instanceKey=<key>

#[derive(Debug, Deserialize)]
pub struct ListManualTasksQuery {
    pub state: Option<String>,
    #[serde(rename = "instanceKey")]
    pub instance_key: Option<i64>,
}

#[derive(Debug, Serialize)]
pub struct ManualTaskDto {
    pub key: i64,
    pub instance_key: i64,
    pub element_instance_key: i64,
    pub element_id: String,
    pub state: String,
    pub variables: Value,
    pub created_at: String,
    pub completed_at: Option<String>,
}

pub async fn list_manual_tasks(
    State(state): State<AppState>,
    Query(query): Query<ListManualTasksQuery>,
) -> impl IntoResponse {
    match manual_tasks::list_manual_tasks(
        &state.store.pool,
        query.state.as_deref(),
        query.instance_key,
    )
    .await
    {
        Ok(rows) => {
            let dtos: Vec<ManualTaskDto> = rows
                .into_iter()
                .map(|r| {
                    let variables: Value =
                        serde_json::from_str(&r.variables).unwrap_or(Value::Object(Default::default()));
                    ManualTaskDto {
                        key: r.key,
                        instance_key: r.instance_key,
                        element_instance_key: r.element_instance_key,
                        element_id: r.element_id,
                        state: r.state,
                        variables,
                        created_at: r.created_at,
                        completed_at: r.completed_at,
                    }
                })
                .collect();
            Json(serde_json::json!({"manualTasks": dtos})).into_response()
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

// POST /api/v1/manual-tasks/{key}/completion

#[derive(Debug, Deserialize)]
pub struct CompleteManualTaskRequest {
    #[serde(default)]
    pub variables: HashMap<String, Value>,
}

pub async fn complete_manual_task(
    State(state): State<AppState>,
    Path(key): Path<i64>,
    Json(body): Json<CompleteManualTaskRequest>,
) -> impl IntoResponse {
    match engine_mt::complete_manual_task(&state.store, key, body.variables).await {
        Ok(status) => Json(serde_json::json!({"status": status})).into_response(),
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
            if msg.contains("not open") =>
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

// POST /api/v1/manual-tasks/{key}/cancellation

#[derive(Debug, Deserialize)]
pub struct CancelManualTaskRequest {
    #[serde(default)]
    pub reason: String,
}

pub async fn cancel_manual_task(
    State(state): State<AppState>,
    Path(key): Path<i64>,
    Json(body): Json<CancelManualTaskRequest>,
) -> impl IntoResponse {
    match engine_mt::cancel_manual_task(&state.store, key, &body.reason).await {
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
            if msg.contains("not open") =>
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
