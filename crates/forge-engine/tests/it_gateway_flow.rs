// Integration test: deploy a gateway-flow BPMN and run instances to completion.
// Exercises RE-01 through RE-11 end-to-end: deploy → activate → start → step → history → audit.
use forge_engine::{deployment::deploy, instance::start_instance};
use forge_model::Config;
use forge_store::{events, migrate::run_migrations, runtime, Store};
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

const GATEWAY_FLOW: &str =
    include_str!("../../forge-bpmn/tests/fixtures/gateway-flow.bpmn");

const GATEWAY_NO_DEFAULT: &str =
    include_str!("../../forge-bpmn/tests/fixtures/gateway-no-default.bpmn");

// --- Branch A: amount > 100 → takes the high flow → End_high ---

#[tokio::test]
async fn branch_high_completes() {
    let (store, _dir) = setup().await;

    deploy(&store, "gateway-flow.bpmn", GATEWAY_FLOW.as_bytes(), true)
        .await
        .unwrap();

    let mut vars = HashMap::new();
    vars.insert("amount".to_string(), Value::from(150));

    let started = start_instance(&store, "gateway-flow", vars).await.unwrap();
    assert_eq!(started.status, "completed", "Expected completed for amount=150");

    // Instance should be in history.
    let hist = runtime::get_instance_history(&store.pool, started.instance_key)
        .await
        .unwrap()
        .expect("Instance should be in history");
    assert_eq!(hist.status, "completed");

    // Element history should include End_high, not End_low.
    let elements = runtime::list_element_instances_history(&store.pool, started.instance_key)
        .await
        .unwrap();
    let ids: Vec<&str> = elements.iter().map(|e| e.element_id.as_str()).collect();
    assert!(ids.contains(&"End_high"), "Should have traversed End_high: {ids:?}");
    assert!(!ids.contains(&"End_low"), "Should NOT have traversed End_low: {ids:?}");

    // Audit events should include instance.started and instance.completed.
    let evts = events::list_events_for_instance(&store.pool, started.instance_key)
        .await
        .unwrap();
    let types: Vec<&str> = evts.iter().map(|e| e.event_type.as_str()).collect();
    assert!(types.contains(&"instance.started"), "Missing instance.started: {types:?}");
    assert!(types.contains(&"instance.completed"), "Missing instance.completed: {types:?}");

    store.close().await;
}

// --- Branch B: amount <= 100 → takes the default (low) flow → End_low ---

#[tokio::test]
async fn branch_low_default_completes() {
    let (store, _dir) = setup().await;

    deploy(&store, "gateway-flow.bpmn", GATEWAY_FLOW.as_bytes(), true)
        .await
        .unwrap();

    let mut vars = HashMap::new();
    vars.insert("amount".to_string(), Value::from(50));

    let started = start_instance(&store, "gateway-flow", vars).await.unwrap();
    assert_eq!(started.status, "completed");

    let elements = runtime::list_element_instances_history(&store.pool, started.instance_key)
        .await
        .unwrap();
    let ids: Vec<&str> = elements.iter().map(|e| e.element_id.as_str()).collect();
    assert!(ids.contains(&"End_low"), "Should have traversed End_low: {ids:?}");
    assert!(!ids.contains(&"End_high"), "Should NOT have traversed End_high: {ids:?}");

    store.close().await;
}

// --- No variable set → condition eval fails → Incident (expression error before default) ---

#[tokio::test]
async fn no_variable_raises_incident() {
    let (store, _dir) = setup().await;

    deploy(&store, "gateway-flow.bpmn", GATEWAY_FLOW.as_bytes(), true)
        .await
        .unwrap();

    // No "amount" variable — condition `amount > 100` eval errors → Incident.
    // Per spec: condition eval errors raise an incident with the offending flow id.
    // The default flow is NOT reached because the error fires first.
    let started = start_instance(&store, "gateway-flow", HashMap::new())
        .await
        .unwrap();
    assert_eq!(
        started.status, "active",
        "Expected active (parked on incident) when condition eval errors"
    );

    // Incident event should be in the journal.
    let evts = events::list_events_for_instance(&store.pool, started.instance_key)
        .await
        .unwrap();
    let types: Vec<&str> = evts.iter().map(|e| e.event_type.as_str()).collect();
    assert!(
        types.contains(&"incident.created"),
        "Expected incident.created event: {types:?}"
    );

    store.close().await;
}

// --- No match + no default → Incident (expression eval error on first flow) ---

#[tokio::test]
async fn no_match_no_default_incident() {
    let (store, _dir) = setup().await;

    deploy(
        &store,
        "gateway-no-default.bpmn",
        GATEWAY_NO_DEFAULT.as_bytes(),
        true,
    )
    .await
    .unwrap();

    // No variable → condition on first flow (`amount > 100`) eval errors → Incident.
    let started = start_instance(&store, "gateway-no-default", HashMap::new())
        .await
        .unwrap();
    assert_eq!(
        started.status, "active",
        "Expected active (parked on incident) when condition eval errors"
    );

    let evts = events::list_events_for_instance(&store.pool, started.instance_key)
        .await
        .unwrap();
    let types: Vec<&str> = evts.iter().map(|e| e.event_type.as_str()).collect();
    assert!(
        types.contains(&"incident.created"),
        "Expected incident.created event: {types:?}"
    );

    store.close().await;
}

// --- Two successive deployments; second is activated ---

#[tokio::test]
async fn second_deploy_version_2_starts() {
    let (store, _dir) = setup().await;

    deploy(&store, "gateway-flow.bpmn", GATEWAY_FLOW.as_bytes(), true)
        .await
        .unwrap();
    let def2 = deploy(&store, "gateway-flow.bpmn", GATEWAY_FLOW.as_bytes(), true)
        .await
        .unwrap();
    assert_eq!(def2.version, 2);

    let mut vars = HashMap::new();
    vars.insert("amount".to_string(), Value::from(200));
    let started = start_instance(&store, "gateway-flow", vars).await.unwrap();
    assert_eq!(started.version, 2);
    assert_eq!(started.status, "completed");

    store.close().await;
}
