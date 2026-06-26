use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use forge_store::definitions::DefinitionRepo;

use crate::state::AppState;

fn to_dto(row: forge_store::definitions::ProcessDefinitionRow, include_xml: bool) -> serde_json::Value {
    let mut v = serde_json::json!({
        "key": row.key,
        "bpmn_process_id": row.bpmn_process_id,
        "version": row.version,
        "resource_name": row.resource_name,
        "deployed_at": row.deployed_at,
        "is_active": row.is_active != 0,
        "checksum": row.checksum,
    });
    if include_xml {
        v["bpmn_xml"] = serde_json::Value::String(row.bpmn_xml);
        v["runtime_graph"] =
            serde_json::from_str(&row.runtime_graph).unwrap_or(serde_json::Value::Null);
    }
    v
}

pub async fn list_definitions(State(state): State<AppState>) -> impl IntoResponse {
    let repo = DefinitionRepo::new(&state.store.pool);
    match repo.list().await {
        Ok(rows) => {
            let dtos: Vec<_> = rows.into_iter().map(|r| to_dto(r, false)).collect();
            Json(serde_json::json!({ "process_definitions": dtos })).into_response()
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": e.to_string() })),
        )
            .into_response(),
    }
}

pub async fn get_definition(
    State(state): State<AppState>,
    Path(key): Path<i64>,
) -> impl IntoResponse {
    let repo = DefinitionRepo::new(&state.store.pool);
    match repo.get_by_key(key).await {
        Ok(Some(row)) => Json(to_dto(row, true)).into_response(),
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({ "error": format!("Definition {key} not found") })),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": e.to_string() })),
        )
            .into_response(),
    }
}

pub async fn activate_definition(
    State(state): State<AppState>,
    Path(key): Path<i64>,
) -> impl IntoResponse {
    let repo = DefinitionRepo::new(&state.store.pool);
    match repo.activate(key).await {
        Ok(()) => Json(serde_json::json!({ "activated": key })).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": e.to_string() })),
        )
            .into_response(),
    }
}
