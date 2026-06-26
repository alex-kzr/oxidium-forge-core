use axum::{
    extract::{Multipart, Query, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use forge_engine::deployment::{deploy, DeployError};
use serde::{Deserialize, Serialize};

use crate::state::AppState;

#[derive(Debug, Deserialize)]
pub struct DeployQuery {
    pub activate: Option<bool>,
}

#[derive(Debug, Serialize)]
pub struct DeployResponse {
    pub deployments: Vec<DeployedDto>,
}

#[derive(Debug, Serialize)]
pub struct DeployedDto {
    pub key: i64,
    pub bpmn_process_id: String,
    pub version: i64,
    pub resource_name: String,
    pub is_active: bool,
}

pub async fn post_deployment(
    State(state): State<AppState>,
    Query(params): Query<DeployQuery>,
    mut multipart: Multipart,
) -> impl IntoResponse {
    let auto_activate = params.activate.unwrap_or(true);
    let mut results = Vec::new();
    let mut errors: Vec<serde_json::Value> = Vec::new();

    while let Ok(Some(field)) = multipart.next_field().await {
        let filename = field.file_name().unwrap_or("upload.bpmn").to_string();
        let data = match field.bytes().await {
            Ok(d) => d,
            Err(e) => {
                errors.push(serde_json::json!({
                    "resource": filename,
                    "error": e.to_string(),
                }));
                continue;
            }
        };

        match deploy(&state.store, &filename, &data, auto_activate).await {
            Ok(def) => results.push(DeployedDto {
                key: def.key,
                bpmn_process_id: def.bpmn_process_id,
                version: def.version,
                resource_name: def.resource_name,
                is_active: def.is_active,
            }),
            Err(DeployError::Validation { count, diagnostics }) => {
                errors.push(serde_json::json!({
                    "resource": filename,
                    "error": "validation_failed",
                    "diagnostic_count": count,
                    "diagnostics": diagnostics,
                }));
            }
            Err(e) => {
                errors.push(serde_json::json!({
                    "resource": filename,
                    "error": e.to_string(),
                }));
            }
        }
    }

    if !errors.is_empty() && results.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({ "errors": errors })),
        )
            .into_response();
    }

    (
        StatusCode::CREATED,
        Json(serde_json::json!({
            "deployments": results,
            "errors": errors,
        })),
    )
        .into_response()
}
