use axum::{
    routing::{get, post},
    Router,
};
use tower_http::trace::TraceLayer;

use crate::{definitions, deployments, health, incidents, instances, jobs, manual_tasks, state::AppState};

pub fn create_router(state: AppState) -> Router {
    Router::new()
        .route("/health", get(health::handler))
        // Deployments & definitions
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
        // Process instances
        .route("/api/v1/process-instances", post(instances::post_instance))
        .route(
            "/api/v1/process-instances/:key",
            get(instances::get_instance),
        )
        .route(
            "/api/v1/process-instances/:key/events",
            get(instances::get_instance_events),
        )
        // Jobs (SJ-09)
        .route("/api/v1/jobs/activation", post(jobs::activate_jobs))
        .route(
            "/api/v1/jobs/:key/completion",
            post(jobs::complete_job),
        )
        .route("/api/v1/jobs/:key/failure", post(jobs::fail_job))
        // Incidents (SJ-09)
        .route("/api/v1/incidents", get(incidents::list_incidents))
        .route(
            "/api/v1/incidents/:key/resolution",
            post(incidents::resolve_incident),
        )
        // Manual tasks (MT-05)
        .route("/api/v1/manual-tasks", get(manual_tasks::list_manual_tasks))
        .route(
            "/api/v1/manual-tasks/:key/completion",
            post(manual_tasks::complete_manual_task),
        )
        .route(
            "/api/v1/manual-tasks/:key/cancellation",
            post(manual_tasks::cancel_manual_task),
        )
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}
