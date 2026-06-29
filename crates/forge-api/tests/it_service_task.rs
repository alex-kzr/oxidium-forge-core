// Integration test: service-task flow driven by a simulated REST worker.
// Exercises Phase 4 end-to-end: deploy → start → activate job → complete → history + audit.
// Also exercises the failure/retry/dead/incident path.
use forge_engine::{deployment::deploy, instance::start_instance, jobs as engine_jobs};
use forge_model::Config;
use forge_store::{
    events, incidents, jobs,
    migrate::run_migrations,
    runtime, Store,
};
use serde_json::Value;
use std::collections::HashMap;
use tempfile::tempdir;

async fn setup() -> (Store, tempfile::TempDir) {
    let dir = tempdir().unwrap();
    let mut config = Config::default();
    config.db_path = dir.path().join("test.db");
    let store = Store::connect(&config).await.unwrap();
    run_migrations(&store.pool).await.unwrap();
    (store, dir)
}

const SERVICE_TASK_BPMN: &str =
    include_str!("../../forge-bpmn/tests/fixtures/simple-service-task.bpmn");

// --- Happy path: deploy → start → activate → complete → completed ---

#[tokio::test]
async fn happy_path_completes() {
    let (store, _dir) = setup().await;

    deploy(&store, "simple-service-task.bpmn", SERVICE_TASK_BPMN.as_bytes(), true)
        .await
        .unwrap();

    // Start with an input variable.
    let mut vars = HashMap::new();
    vars.insert("inputData".to_string(), Value::from("hello"));
    let started = start_instance(&store, "simple-service-task", vars).await.unwrap();
    assert_eq!(started.status, "active", "Instance should be waiting on job");

    // The instance is parked; there should be one activatable job.
    let job_list = jobs::list_jobs_for_instance(&store.pool, started.instance_key)
        .await
        .unwrap();
    assert_eq!(job_list.len(), 1, "Expected one job");
    let job = &job_list[0];
    assert_eq!(job.state, "activatable");
    assert_eq!(job.task_type, "work-handler");

    // Input mapping: job variables should contain 'workInput' (mapped from inputData).
    let job_vars: Value = serde_json::from_str(&job.variables).unwrap();
    assert_eq!(
        job_vars.get("workInput"),
        Some(&Value::from("hello")),
        "Input mapping should have set workInput"
    );

    // Activate the job.
    let mut conn = store.pool.acquire().await.unwrap();
    let activated = jobs::activate_jobs(&mut *conn, "work-handler", "test-worker", 1, 60)
        .await
        .unwrap();
    drop(conn);
    assert_eq!(activated.len(), 1);
    let job_key = activated[0].key;

    // Complete the job with an output variable.
    let mut output = HashMap::new();
    output.insert("result".to_string(), Value::from("done"));
    engine_jobs::complete_job(&store, job_key, output).await.unwrap();

    // Instance should now be completed and in history.
    let hist = runtime::get_instance_history(&store.pool, started.instance_key)
        .await
        .unwrap()
        .expect("Instance should be in history");
    assert_eq!(hist.status, "completed");

    // Output mapping: workResult should be in history variables.
    let hist_vars: Vec<(String, String)> = sqlx::query_as(
        "SELECT name, value FROM variables_history WHERE instance_key = ?",
    )
    .bind(started.instance_key)
    .fetch_all(&store.pool)
    .await
    .unwrap();
    let var_map: HashMap<String, Value> = hist_vars
        .into_iter()
        .map(|(k, v)| (k, serde_json::from_str::<Value>(&v).unwrap()))
        .collect();
    assert_eq!(
        var_map.get("workResult"),
        Some(&Value::from("done")),
        "Output mapping should have set workResult"
    );

    // Audit events should include job.created and job.completed.
    let evts = events::list_events_for_instance(&store.pool, started.instance_key)
        .await
        .unwrap();
    let types: Vec<&str> = evts.iter().map(|e| e.event_type.as_str()).collect();
    assert!(types.contains(&"job.created"), "Missing job.created: {types:?}");
    assert!(types.contains(&"job.completed"), "Missing job.completed: {types:?}");
    assert!(types.contains(&"instance.completed"), "Missing instance.completed: {types:?}");

    store.close().await;
}

// --- Failure → retry → dead → incident → resolve → complete ---

#[tokio::test]
async fn failure_retry_dead_incident_resolve() {
    let (store, _dir) = setup().await;

    deploy(&store, "simple-service-task.bpmn", SERVICE_TASK_BPMN.as_bytes(), true)
        .await
        .unwrap();

    let mut vars = HashMap::new();
    vars.insert("inputData".to_string(), Value::from("test"));
    let started = start_instance(&store, "simple-service-task", vars)
        .await
        .unwrap();
    assert_eq!(started.status, "active");

    let mut conn = store.pool.acquire().await.unwrap();

    // First activation + fail with retries=2.
    let activated = jobs::activate_jobs(&mut *conn, "work-handler", "w", 1, 60)
        .await
        .unwrap();
    drop(conn);
    let job_key = activated[0].key;
    engine_jobs::fail_job(&store, job_key, "transient error", 2, None).await.unwrap();

    let job = jobs::get_job(&store.pool, job_key).await.unwrap().unwrap();
    assert_eq!(job.state, "activatable", "Job should be re-activatable");
    assert_eq!(job.retries, 2);

    // Second activation + fail with retries=1.
    let mut conn = store.pool.acquire().await.unwrap();
    let activated = jobs::activate_jobs(&mut *conn, "work-handler", "w", 1, 60)
        .await
        .unwrap();
    drop(conn);
    let job_key = activated[0].key;
    engine_jobs::fail_job(&store, job_key, "still broken", 1, None).await.unwrap();

    // Third activation + fail with retries=0 → dead + incident.
    let mut conn = store.pool.acquire().await.unwrap();
    let activated = jobs::activate_jobs(&mut *conn, "work-handler", "w", 1, 60)
        .await
        .unwrap();
    drop(conn);
    let job_key = activated[0].key;
    engine_jobs::fail_job(&store, job_key, "fatal error", 0, None).await.unwrap();

    let job = jobs::get_job(&store.pool, job_key).await.unwrap().unwrap();
    assert_eq!(job.state, "dead", "Job should be dead");

    let incident_list = incidents::list_incidents(&store.pool, Some("active")).await.unwrap();
    assert_eq!(incident_list.len(), 1, "Expected one active incident");
    let incident = &incident_list[0];
    assert_eq!(incident.incident_type, "retry-exhausted");
    let incident_key = incident.key;

    // Instance is still active (parked on incident).
    let inst = runtime::get_instance(&store.pool, started.instance_key)
        .await
        .unwrap()
        .expect("Instance should still be active");
    assert_eq!(inst.status, "active");

    // Resolve the incident → job becomes activatable again.
    engine_jobs::resolve_incident(&store, incident_key).await.unwrap();

    let job = jobs::get_job(&store.pool, job_key).await.unwrap().unwrap();
    assert_eq!(job.state, "activatable", "Job should be activatable after resolve");

    let incident_list = incidents::list_incidents(&store.pool, Some("active")).await.unwrap();
    assert_eq!(incident_list.len(), 0, "No active incidents after resolve");

    // Now complete successfully.
    let mut conn = store.pool.acquire().await.unwrap();
    let activated = jobs::activate_jobs(&mut *conn, "work-handler", "w", 1, 60)
        .await
        .unwrap();
    drop(conn);
    let job_key = activated[0].key;

    let mut output = HashMap::new();
    output.insert("result".to_string(), Value::from("recovered"));
    engine_jobs::complete_job(&store, job_key, output).await.unwrap();

    let hist = runtime::get_instance_history(&store.pool, started.instance_key)
        .await
        .unwrap()
        .expect("Instance should be in history after completion");
    assert_eq!(hist.status, "completed");

    // Audit should contain incident.created and incident.resolved.
    let evts = events::list_events_for_instance(&store.pool, started.instance_key)
        .await
        .unwrap();
    let types: Vec<&str> = evts.iter().map(|e| e.event_type.as_str()).collect();
    assert!(types.contains(&"incident.created"), "Missing incident.created: {types:?}");
    assert!(types.contains(&"incident.resolved"), "Missing incident.resolved: {types:?}");

    store.close().await;
}

// --- Concurrent activation: two workers don't get the same job ---

#[tokio::test]
async fn concurrent_activation_disjoint() {
    let (store, _dir) = setup().await;

    deploy(&store, "simple-service-task.bpmn", SERVICE_TASK_BPMN.as_bytes(), true)
        .await
        .unwrap();

    // Start two instances → two jobs.
    let mut vars = HashMap::new();
    vars.insert("inputData".to_string(), Value::from("x"));
    start_instance(&store, "simple-service-task", vars.clone()).await.unwrap();
    start_instance(&store, "simple-service-task", vars).await.unwrap();

    // Two separate activations.
    let mut conn1 = store.pool.acquire().await.unwrap();
    let a1 = jobs::activate_jobs(&mut *conn1, "work-handler", "worker-1", 1, 60)
        .await
        .unwrap();
    drop(conn1);

    let mut conn2 = store.pool.acquire().await.unwrap();
    let a2 = jobs::activate_jobs(&mut *conn2, "work-handler", "worker-2", 1, 60)
        .await
        .unwrap();
    drop(conn2);

    assert_eq!(a1.len(), 1);
    assert_eq!(a2.len(), 1);
    assert_ne!(
        a1[0].key, a2[0].key,
        "Workers should get distinct jobs"
    );

    store.close().await;
}
