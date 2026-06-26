use crate::model::*;
use std::collections::{HashSet, VecDeque};

#[derive(Debug, Clone)]
pub struct ValidationDiagnostic {
    pub element_id: Option<String>,
    pub code: String,
    pub message: String,
}

pub fn validate(process: &ParsedProcess) -> Vec<ValidationDiagnostic> {
    let mut errors = Vec::new();

    // 1. Unsupported elements
    for node in &process.nodes {
        if let Node::Unsupported(n) = node {
            errors.push(ValidationDiagnostic {
                element_id: Some(n.id.clone()),
                code: "UNSUPPORTED_ELEMENT".to_string(),
                message: format!(
                    "Element type '{}' is not implemented in this engine",
                    n.element_type
                ),
            });
        }
    }

    // 2. Exactly one start event
    let start_events: Vec<_> = process
        .nodes
        .iter()
        .filter(|n| matches!(n, Node::StartEvent(_)))
        .collect();
    if start_events.len() != 1 {
        errors.push(ValidationDiagnostic {
            element_id: None,
            code: "START_EVENT_COUNT".to_string(),
            message: format!(
                "Process must have exactly one start event, found {}",
                start_events.len()
            ),
        });
    }

    // 3. At least one end event
    let end_events: Vec<_> = process
        .nodes
        .iter()
        .filter(|n| matches!(n, Node::EndEvent(_)))
        .collect();
    if end_events.is_empty() {
        errors.push(ValidationDiagnostic {
            element_id: None,
            code: "END_EVENT_MISSING".to_string(),
            message: "Process must have at least one end event".to_string(),
        });
    }

    // Build set of node ids.
    let node_ids: HashSet<&str> = process.nodes.iter().map(|n| n.id()).collect();

    // 4. No dangling sourceRef/targetRef in flows
    for flow in &process.flows {
        if !node_ids.contains(flow.source_ref.as_str()) {
            errors.push(ValidationDiagnostic {
                element_id: Some(flow.id.clone()),
                code: "INVALID_FLOW_REF".to_string(),
                message: format!(
                    "Sequence flow '{}' has unknown sourceRef '{}'",
                    flow.id, flow.source_ref
                ),
            });
        }
        if !node_ids.contains(flow.target_ref.as_str()) {
            errors.push(ValidationDiagnostic {
                element_id: Some(flow.id.clone()),
                code: "INVALID_FLOW_REF".to_string(),
                message: format!(
                    "Sequence flow '{}' has unknown targetRef '{}'",
                    flow.id, flow.target_ref
                ),
            });
        }
    }

    // 5. Service tasks must have non-empty task type
    for node in &process.nodes {
        if let Node::ServiceTask(t) = node {
            if t.task_type.trim().is_empty() {
                errors.push(ValidationDiagnostic {
                    element_id: Some(t.id.clone()),
                    code: "MISSING_TASK_TYPE".to_string(),
                    message: format!("Service task '{}' has no task type defined", t.id),
                });
            }
        }
    }

    // 6. Exclusive gateways: condition + default constraints
    for node in &process.nodes {
        if let Node::ExclusiveGateway(g) = node {
            let outgoing: Vec<&SequenceFlow> = process
                .flows
                .iter()
                .filter(|f| f.source_ref == g.id)
                .collect();

            let default_count = outgoing.iter().filter(|f| f.is_default).count();
            if default_count > 1 {
                errors.push(ValidationDiagnostic {
                    element_id: Some(g.id.clone()),
                    code: "MULTIPLE_DEFAULT_FLOWS".to_string(),
                    message: format!(
                        "Exclusive gateway '{}' has {} default flows, at most one allowed",
                        g.id, default_count
                    ),
                });
            }

            // Each outgoing flow except the default must have a condition.
            for flow in &outgoing {
                if !flow.is_default && flow.condition.is_none() && outgoing.len() > 1 {
                    errors.push(ValidationDiagnostic {
                        element_id: Some(flow.id.clone()),
                        code: "MISSING_CONDITION".to_string(),
                        message: format!(
                            "Outgoing flow '{}' from gateway '{}' has no condition and is not the default",
                            flow.id, g.id
                        ),
                    });
                }
            }
        }
    }

    // 7. Graph connectivity — all nodes reachable from the start event.
    if start_events.len() == 1 {
        let start_id = start_events[0].id();
        let mut adjacency: std::collections::HashMap<&str, Vec<&str>> =
            std::collections::HashMap::new();
        for flow in &process.flows {
            adjacency
                .entry(flow.source_ref.as_str())
                .or_default()
                .push(flow.target_ref.as_str());
        }

        let mut visited: HashSet<&str> = HashSet::new();
        let mut queue: VecDeque<&str> = VecDeque::new();
        queue.push_back(start_id);
        visited.insert(start_id);
        while let Some(cur) = queue.pop_front() {
            if let Some(targets) = adjacency.get(cur) {
                for t in targets {
                    if visited.insert(*t) {
                        queue.push_back(*t);
                    }
                }
            }
        }

        for node in &process.nodes {
            if !visited.contains(node.id()) {
                errors.push(ValidationDiagnostic {
                    element_id: Some(node.id().to_string()),
                    code: "UNREACHABLE_NODE".to_string(),
                    message: format!(
                        "Node '{}' is not reachable from the start event",
                        node.id()
                    ),
                });
            }
        }
    }

    errors
}
