use serde::{Deserialize, Serialize};
use std::collections::HashMap;

pub const GRAPH_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeGraph {
    pub schema_version: u32,
    pub process_id: String,
    pub start_node: String,
    pub nodes: HashMap<String, CompiledNode>,
    pub outgoing: HashMap<String, Vec<CompiledFlow>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompiledNode {
    pub id: String,
    pub kind: NodeKind,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind")]
pub enum NodeKind {
    StartEvent,
    EndEvent,
    ExclusiveGateway {
        default_flow: Option<String>,
    },
    ServiceTask {
        task_type: String,
        retries: u32,
        io_mapping: Option<CompiledIoMapping>,
    },
    ManualTask {
        io_mapping: Option<CompiledIoMapping>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompiledFlow {
    pub id: String,
    pub target: String,
    pub condition: Option<String>,
    pub is_default: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CompiledIoMapping {
    pub inputs: Vec<CompiledMapping>,
    pub outputs: Vec<CompiledMapping>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompiledMapping {
    pub source: String,
    pub target: String,
}
