use axum::{
    routing::{get, post},
    Router,
};
use tower_http::trace::TraceLayer;

use crate::{definitions, deployments, health, instances, state::AppState};

pub fn create_router(state: AppState) -> Router {
    Router::new()
        .route("/health", get(health::handler))
        .route("/api/v1/deployments", post(deployments::post_deployment))
        .route(
            "/api/v1/process-definitions",
            get(definitions::list_definitions),
        )
        .route(
            "/api/v1/process-definitions/:key",
            get(definitions::get_definition),
        )
        .route(
            "/api/v1/process-definitions/:key/activation",
            post(definitions::activate_definition),
        )
        .route(
            "/api/v1/process-instances",
            post(instances::post_instance),
        )
        .route(
            "/api/v1/process-instances/:key",
            get(instances::get_instance),
        )
        .route(
            "/api/v1/process-instances/:key/events",
            get(instances::get_instance_events),
        )
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}
