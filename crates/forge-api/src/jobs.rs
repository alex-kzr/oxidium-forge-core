use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use forge_engine::jobs as engine_jobs;
use forge_store::jobs;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;

use crate::state::AppState;

// POST /api/v1/jobs/activation

#[derive(Debug, Deserialize)]
pub struct ActivateJobsRequest {
    #[serde(rename = "taskType")]
    pub task_type: String,
    #[serde(default = "default_worker")]
    pub worker: String,
    #[serde(rename = "maxJobs", default = "default_max_jobs")]
    pub max_jobs: i64,
    #[serde(rename = "lockDuration", default = "default_lock_duration")]
    pub lock_duration: i64,
}

fn default_worker() -> String {
    "default".to_string()
}
fn default_max_jobs() -> i64 {
    1
}
fn default_lock_duration() -> i64 {
    60
}

#[derive(Debug, Serialize)]
pub struct ActivatedJob {
    pub key: i64,
    pub instance_key: i64,
    pub element_id: String,
    pub task_type: String,
    pub retries: i64,
    pub worker: String,
    pub locked_until: Option<String>,
    pub variables: Value,
}

pub async fn activate_jobs(
    State(state): State<AppState>,
    Json(body): Json<ActivateJobsRequest>,
) -> impl IntoResponse {
    let mut conn = match state.store.pool.acquire().await {
        Ok(c) => c,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": e.to_string()})),
            )
                .into_response();
        }
    };

    match jobs::activate_jobs(
        &mut *conn,
        &body.task_type,
        &body.worker,
        body.max_jobs,
        body.lock_duration,
    )
    .await
    {
        Ok(rows) => {
            let activated: Vec<ActivatedJob> = rows
                .into_iter()
                .map(|r| {
                    let variables: Value =
                        serde_json::from_str(&r.variables).unwrap_or(Value::Object(Default::default()));
                    ActivatedJob {
                        key: r.key,
                        instance_key: r.instance_key,
                        element_id: r.element_id,
                        task_type: r.task_type,
                        retries: r.retries,
                        worker: r.worker.unwrap_or_default(),
                        locked_until: r.locked_until,
                        variables,
                    }
                })
                .collect();
            Json(serde_json::json!({"jobs": activated})).into_response()
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

// POST /api/v1/jobs/{key}/completion

#[derive(Debug, Deserialize)]
pub struct CompleteJobRequest {
    #[serde(default)]
    pub variables: HashMap<String, Value>,
}

pub async fn complete_job(
    State(state): State<AppState>,
    Path(key): Path<i64>,
    Json(body): Json<CompleteJobRequest>,
) -> impl IntoResponse {
    match engine_jobs::complete_job(&state.store, key, body.variables).await {
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
            if msg.contains("not activated") =>
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

// POST /api/v1/jobs/{key}/failure

#[derive(Debug, Deserialize)]
pub struct FailJobRequest {
    #[serde(rename = "errorMessage")]
    pub error_message: String,
    pub retries: i64,
    #[serde(rename = "retryBackoff")]
    pub retry_backoff: Option<i64>,
}

pub async fn fail_job(
    State(state): State<AppState>,
    Path(key): Path<i64>,
    Json(body): Json<FailJobRequest>,
) -> impl IntoResponse {
    match engine_jobs::fail_job(
        &state.store,
        key,
        &body.error_message,
        body.retries,
        body.retry_backoff,
    )
    .await
    {
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
            if msg.contains("not activated") =>
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
