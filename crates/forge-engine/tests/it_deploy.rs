// Integration test: deploy a real Modeler 8 diagram via the deployment service.
use forge_engine::deployment::deploy;
use forge_model::Config;
use forge_store::{migrate::run_migrations, Store};
use tempfile::tempdir;

#[tokio::test]
async fn deploy_simple_service_task() {
    let dir = tempdir().unwrap();
    let mut config = Config::default();
    config.db_path = dir.path().join("test.db");

    let store = Store::connect(&config).await.unwrap();
    run_migrations(&store.pool).await.unwrap();

    let xml = include_str!("../../forge-bpmn/tests/fixtures/simple-service-task.bpmn");
    let result = deploy(&store, "simple-service-task.bpmn", xml.as_bytes(), true)
        .await
        .unwrap();

    assert_eq!(result.bpmn_process_id, "simple-service-task");
    assert_eq!(result.version, 1);
    assert!(result.is_active);

    // Deploy again — should be version 2, not active.
    let result2 = deploy(&store, "simple-service-task.bpmn", xml.as_bytes(), false)
        .await
        .unwrap();
    assert_eq!(result2.version, 2);
    assert!(!result2.is_active);

    store.close().await;
}
