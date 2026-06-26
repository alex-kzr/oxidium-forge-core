use crate::graph::*;
use crate::model::*;
use std::collections::HashMap;

pub fn compile(process: &ParsedProcess) -> RuntimeGraph {
    let start_node = process
        .nodes
        .iter()
        .find(|n| matches!(n, Node::StartEvent(_)))
        .map(|n| n.id().to_string())
        .unwrap_or_default();

    let nodes: HashMap<String, CompiledNode> = process
        .nodes
        .iter()
        .filter_map(|n| {
            let kind = match n {
                Node::StartEvent(_) => Some(NodeKind::StartEvent),
                Node::EndEvent(_) => Some(NodeKind::EndEvent),
                Node::ExclusiveGateway(g) => Some(NodeKind::ExclusiveGateway {
                    default_flow: g.default_flow.clone(),
                }),
                Node::ServiceTask(t) => Some(NodeKind::ServiceTask {
                    task_type: t.task_type.clone(),
                    retries: t.retries,
                    io_mapping: t.io_mapping.as_ref().map(compile_io_mapping),
                }),
                Node::ManualTask(t) => Some(NodeKind::ManualTask {
                    io_mapping: t.io_mapping.as_ref().map(compile_io_mapping),
                }),
                Node::Unsupported(_) => None, // skipped; validation caught this
            }?;
            Some((
                n.id().to_string(),
                CompiledNode {
                    id: n.id().to_string(),
                    kind,
                },
            ))
        })
        .collect();

    let mut outgoing: HashMap<String, Vec<CompiledFlow>> = HashMap::new();
    for flow in &process.flows {
        outgoing
            .entry(flow.source_ref.clone())
            .or_default()
            .push(CompiledFlow {
                id: flow.id.clone(),
                target: flow.target_ref.clone(),
                condition: flow.condition.clone(),
                is_default: flow.is_default,
            });
    }

    RuntimeGraph {
        schema_version: GRAPH_SCHEMA_VERSION,
        process_id: process.id.clone(),
        start_node,
        nodes,
        outgoing,
    }
}

fn compile_io_mapping(m: &IoMapping) -> CompiledIoMapping {
    CompiledIoMapping {
        inputs: m
            .inputs
            .iter()
            .map(|i| CompiledMapping {
                source: i.source.clone(),
                target: i.target.clone(),
            })
            .collect(),
        outputs: m
            .outputs
            .iter()
            .map(|o| CompiledMapping {
                source: o.source.clone(),
                target: o.target.clone(),
            })
            .collect(),
    }
}
