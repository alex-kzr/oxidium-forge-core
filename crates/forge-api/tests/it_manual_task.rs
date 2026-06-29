// Integration test: manual-task flow.
// Exercises Phase 5 end-to-end:
//   - Complete path: deploy → start → list open → complete with output → instance completed + history + audit.
//   - Cancel path: start → cancel → instance cancelled + history.
use forge_engine::{deployment::deploy, instance::start_instance, manual_tasks as engine_mt};
use forge_model::Config;
use forge_store::{events, manual_tasks, migrate::run_migrations, runtime, Store};
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

const MANUAL_TASK_BPMN: &str =
    include_str!("../../forge-bpmn/tests/fixtures/simple-manual-task.bpmn");

// --- Happy path: deploy → start → list → complete → completed ---

#[tokio::test]
async fn complete_path() {
    let (store, _dir) = setup().await;

    deploy(&store, "simple-manual-task.bpmn", MANUAL_TASK_BPMN.as_bytes(), true)
        .await
        .unwrap();

    let mut vars = HashMap::new();
    vars.insert("inputData".to_string(), Value::from("review-me"));
    let started = start_instance(&store, "simple-manual-task", vars).await.unwrap();
    assert_eq!(started.status, "active", "Instance should be waiting on manual task");

    // There should be one open manual task.
    let task_list = manual_tasks::list_manual_tasks(&store.pool, Some("open"), None)
        .await
        .unwrap();
    assert_eq!(task_list.len(), 1, "Expected one open manual task");
    let task = &task_list[0];
    assert_eq!(task.state, "open");
    assert_eq!(task.element_id, "Activity_review");
    assert_eq!(task.instance_key, started.instance_key);

    // Input mapping: task variables should contain 'reviewInput'.
    let task_vars: Value = serde_json::from_str(&task.variables).unwrap();
    assert_eq!(
        task_vars.get("reviewInput"),
        Some(&Value::from("review-me")),
        "Input mapping should have set reviewInput"
    );

    // Complete the task with an output variable.
    let task_key = task.key;
    let mut output = HashMap::new();
    output.insert("decision".to_string(), Value::from("approved"));
    let status = engine_mt::complete_manual_task(&store, task_key, output)
        .await
        .unwrap();
    assert_eq!(status, "completed");

    // Instance should be in history as completed.
    let hist = runtime::get_instance_history(&store.pool, started.instance_key)
        .await
        .unwrap()
        .expect("Instance should be in history");
    assert_eq!(hist.status, "completed");

    // Output mapping: reviewDecision should be in history variables.
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
        var_map.get("reviewDecision"),
        Some(&Value::from("approved")),
        "Output mapping should have set reviewDecision"
    );

    // Manual task should be in history with state=completed.
    let mt_hist: Vec<(String,)> = sqlx::query_as(
        "SELECT state FROM manual_tasks_history WHERE instance_key = ?",
    )
    .bind(started.instance_key)
    .fetch_all(&store.pool)
    .await
    .unwrap();
    assert_eq!(mt_hist.len(), 1);
    assert_eq!(mt_hist[0].0, "completed");

    // Audit should include manual_task.created and manual_task.completed.
    let evts = events::list_events_for_instance(&store.pool, started.instance_key)
        .await
        .unwrap();
    let types: Vec<&str> = evts.iter().map(|e| e.event_type.as_str()).collect();
    assert!(
        types.contains(&"manual_task.created"),
        "Missing manual_task.created: {types:?}"
    );
    assert!(
        types.contains(&"manual_task.completed"),
        "Missing manual_task.completed: {types:?}"
    );
    assert!(
        types.contains(&"instance.completed"),
        "Missing instance.completed: {types:?}"
    );

    store.close().await;
}

// --- Cancel path: start → cancel → instance cancelled + history ---

#[tokio::test]
async fn cancel_path() {
    let (store, _dir) = setup().await;

    deploy(&store, "simple-manual-task.bpmn", MANUAL_TASK_BPMN.as_bytes(), true)
        .await
        .unwrap();

    let mut init_vars = HashMap::new();
    init_vars.insert("inputData".to_string(), Value::from("cancel-me"));
    let started = start_instance(&store, "simple-manual-task", init_vars)
        .await
        .unwrap();
    assert_eq!(started.status, "active");

    let task_list = manual_tasks::list_manual_tasks(&store.pool, Some("open"), None)
        .await
        .unwrap();
    assert_eq!(task_list.len(), 1);
    let task_key = task_list[0].key;

    engine_mt::cancel_manual_task(&store, task_key, "no longer needed")
        .await
        .unwrap();

    // Instance should be in history as cancelled.
    let hist = runtime::get_instance_history(&store.pool, started.instance_key)
        .await
        .unwrap()
        .expect("Instance should be in history");
    assert_eq!(hist.status, "cancelled");

    // Manual task should be in history with state=cancelled.
    let mt_hist: Vec<(String,)> = sqlx::query_as(
        "SELECT state FROM manual_tasks_history WHERE instance_key = ?",
    )
    .bind(started.instance_key)
    .fetch_all(&store.pool)
    .await
    .unwrap();
    assert_eq!(mt_hist.len(), 1);
    assert_eq!(mt_hist[0].0, "cancelled");

    // Audit should include manual_task.cancelled and instance.cancelled.
    let evts = events::list_events_for_instance(&store.pool, started.instance_key)
        .await
        .unwrap();
    let types: Vec<&str> = evts.iter().map(|e| e.event_type.as_str()).collect();
    assert!(
        types.contains(&"manual_task.cancelled"),
        "Missing manual_task.cancelled: {types:?}"
    );
    assert!(
        types.contains(&"instance.cancelled"),
        "Missing instance.cancelled: {types:?}"
    );

    store.close().await;
}

// --- Error cases: complete/cancel unknown or already-closed task ---

#[tokio::test]
async fn complete_unknown_task_returns_error() {
    let (store, _dir) = setup().await;
    let result = engine_mt::complete_manual_task(&store, 9999, HashMap::new()).await;
    assert!(result.is_err());
    let msg = result.unwrap_err().to_string();
    assert!(msg.contains("not found"), "Expected 'not found' in: {msg}");
    store.close().await;
}

#[tokio::test]
async fn complete_already_completed_task_returns_conflict() {
    let (store, _dir) = setup().await;

    deploy(&store, "simple-manual-task.bpmn", MANUAL_TASK_BPMN.as_bytes(), true)
        .await
        .unwrap();

    let mut init_vars = HashMap::new();
    init_vars.insert("inputData".to_string(), Value::from("data"));
    let started = start_instance(&store, "simple-manual-task", init_vars)
        .await
        .unwrap();

    let task_list = manual_tasks::list_manual_tasks(&store.pool, Some("open"), None)
        .await
        .unwrap();
    let task_key = task_list[0].key;

    let mut output = HashMap::new();
    output.insert("decision".to_string(), Value::from("ok"));
    engine_mt::complete_manual_task(&store, task_key, output)
        .await
        .unwrap();

    // Second completion should fail with "not open" or "not found" (task moved to history).
    let result = engine_mt::complete_manual_task(&store, task_key, HashMap::new()).await;
    assert!(result.is_err());
    let msg = result.unwrap_err().to_string();
    assert!(
        msg.contains("not open") || msg.contains("not found"),
        "Expected conflict error in: {msg}"
    );

    drop(started);
    store.close().await;
}
