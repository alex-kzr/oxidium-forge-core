use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use forge_engine::instance::start_instance;
use forge_store::{events, runtime, variables};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;

use crate::state::AppState;

#[derive(Debug, Deserialize)]
pub struct StartInstanceRequest {
    #[serde(rename = "bpmnProcessId")]
    pub bpmn_process_id: String,
    #[serde(default)]
    pub variables: HashMap<String, Value>,
}

#[derive(Debug, Serialize)]
pub struct StartInstanceResponse {
    pub key: i64,
    pub bpmn_process_id: String,
    pub version: i64,
    pub status: String,
}

pub async fn post_instance(
    State(state): State<AppState>,
    Json(body): Json<StartInstanceRequest>,
) -> impl IntoResponse {
    match start_instance(&state.store, &body.bpmn_process_id, body.variables).await {
        Ok(started) => (
            StatusCode::CREATED,
            Json(serde_json::json!({
                "key": started.instance_key,
                "bpmn_process_id": started.bpmn_process_id,
                "version": started.version,
                "status": started.status,
            })),
        )
            .into_response(),
        Err(forge_engine::EngineError::NoActiveDefinition(id)) => (
            StatusCode::CONFLICT,
            Json(serde_json::json!({
                "error": format!("No active definition for process '{id}'")
            })),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": e.to_string() })),
        )
            .into_response(),
    }
}

pub async fn get_instance(
    State(state): State<AppState>,
    Path(key): Path<i64>,
) -> impl IntoResponse {
    let pool = &state.store.pool;

    // Try runtime tables first.
    let runtime_row = match runtime::get_instance(pool, key).await {
        Ok(row) => row,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "error": e.to_string() })),
            )
                .into_response();
        }
    };

    if let Some(row) = runtime_row {
        // Instance is still running — return live state.
        let elements = runtime::list_active_element_instances(pool, row.key)
            .await
            .unwrap_or_default();
        let vars = variables::get_all_for_instance(pool, row.key)
            .await
            .unwrap_or_default();

        return Json(serde_json::json!({
            "key": row.key,
            "bpmn_process_id": row.bpmn_process_id,
            "version": row.version,
            "status": row.status,
            "started_at": row.started_at,
            "ended_at": row.ended_at,
            "active_elements": elements.iter().map(|e| serde_json::json!({
                "element_id": e.element_id,
                "element_type": e.element_type,
                "state": e.state,
                "entered_at": e.entered_at,
            })).collect::<Vec<_>>(),
            "variables": vars,
        }))
        .into_response();
    }

    // Check history.
    let hist_row = match runtime::get_instance_history(pool, key).await {
        Ok(row) => row,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "error": e.to_string() })),
            )
                .into_response();
        }
    };

    if let Some(hist) = hist_row {
        let elements = runtime::list_element_instances_history(pool, hist.key)
            .await
            .unwrap_or_default();

        return Json(serde_json::json!({
            "key": hist.key,
            "bpmn_process_id": hist.bpmn_process_id,
            "version": hist.version,
            "status": hist.status,
            "started_at": hist.started_at,
            "ended_at": hist.ended_at,
            "active_elements": [],
            "elements_history": elements.iter().map(|e| serde_json::json!({
                "element_id": e.element_id,
                "element_type": e.element_type,
                "state": e.state,
                "entered_at": e.entered_at,
                "left_at": e.left_at,
            })).collect::<Vec<_>>(),
            "variables": {},
        }))
        .into_response();
    }

    (
        StatusCode::NOT_FOUND,
        Json(serde_json::json!({ "error": format!("Instance {key} not found") })),
    )
        .into_response()
}

pub async fn get_instance_events(
    State(state): State<AppState>,
    Path(key): Path<i64>,
) -> impl IntoResponse {
    match events::list_events_for_instance(&state.store.pool, key).await {
        Ok(rows) => {
            let data: Vec<_> = rows
                .into_iter()
                .map(|r| {
                    let payload: Value =
                        serde_json::from_str(&r.payload).unwrap_or(Value::Null);
                    serde_json::json!({
                        "key": r.key,
                        "event_type": r.event_type,
                        "element_id": r.element_id,
                        "payload": payload,
                        "created_at": r.created_at,
                    })
                })
                .collect();
            Json(serde_json::json!({ "events": data })).into_response()
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": e.to_string() })),
        )
            .into_response(),
    }
}
