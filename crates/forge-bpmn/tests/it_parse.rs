use forge_bpmn::model::Node;

#[test]
fn parse_simple_service_task() {
    let xml = include_str!("fixtures/simple-service-task.bpmn");
    let process = forge_bpmn::parse_bpmn(xml.as_bytes()).unwrap();
    assert_eq!(process.id, "simple-service-task");
    assert!(process.executable);

    let start_count = process
        .nodes
        .iter()
        .filter(|n| matches!(n, Node::StartEvent(_)))
        .count();
    let end_count = process
        .nodes
        .iter()
        .filter(|n| matches!(n, Node::EndEvent(_)))
        .count();
    let service_count = process
        .nodes
        .iter()
        .filter(|n| matches!(n, Node::ServiceTask(_)))
        .count();

    assert_eq!(start_count, 1);
    assert_eq!(end_count, 1);
    assert_eq!(service_count, 1);
    assert_eq!(process.flows.len(), 2);

    // Service task should carry its zeebe task definition + io mapping.
    let task = process
        .nodes
        .iter()
        .find_map(|n| match n {
            Node::ServiceTask(t) => Some(t),
            _ => None,
        })
        .unwrap();
    assert_eq!(task.task_type, "work-handler");
    assert_eq!(task.retries, 3);
    let io = task.io_mapping.as_ref().unwrap();
    assert_eq!(io.inputs.len(), 1);
    assert_eq!(io.outputs.len(), 1);
    assert_eq!(io.inputs[0].target, "workInput");

    let errors = forge_bpmn::validate(&process);
    assert!(errors.is_empty(), "Validation errors: {:?}", errors);

    let graph = forge_bpmn::compile(&process);
    let json = serde_json::to_string(&graph).unwrap();
    let graph2: forge_bpmn::RuntimeGraph = serde_json::from_str(&json).unwrap();
    assert_eq!(graph.process_id, graph2.process_id);
    assert_eq!(graph.nodes.len(), graph2.nodes.len());
    assert_eq!(graph.start_node, "StartEvent_1");
}

#[test]
fn parse_exclusive_gateway() {
    let xml = include_str!("fixtures/exclusive-gateway.bpmn");
    let process = forge_bpmn::parse_bpmn(xml.as_bytes()).unwrap();
    assert_eq!(process.id, "exclusive-gateway");

    let gw = process
        .nodes
        .iter()
        .find_map(|n| match n {
            Node::ExclusiveGateway(g) => Some(g),
            _ => None,
        })
        .unwrap();
    assert_eq!(gw.default_flow.as_deref(), Some("Flow_to_low"));

    // The conditional flow should carry its condition; the default should be flagged.
    let high = process.flows.iter().find(|f| f.id == "Flow_to_high").unwrap();
    assert_eq!(high.condition.as_deref(), Some("=amount > 100"));
    let low = process.flows.iter().find(|f| f.id == "Flow_to_low").unwrap();
    assert!(low.is_default);

    let errors = forge_bpmn::validate(&process);
    assert!(errors.is_empty(), "Validation errors: {:?}", errors);
}

#[test]
fn non_executable_process_is_rejected() {
    let xml = r#"<?xml version="1.0"?>
        <bpmn:definitions xmlns:bpmn="http://www.omg.org/spec/BPMN/20100524/MODEL">
          <bpmn:process id="not-exec" isExecutable="false">
            <bpmn:startEvent id="S" />
          </bpmn:process>
        </bpmn:definitions>"#;
    let result = forge_bpmn::parse_bpmn(xml.as_bytes());
    assert!(matches!(
        result,
        Err(forge_bpmn::ParseError::NoExecutableProcess)
    ));
}
