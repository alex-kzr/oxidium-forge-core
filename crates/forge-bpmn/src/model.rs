use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParsedProcess {
    pub id: String,
    pub name: Option<String>,
    pub executable: bool,
    pub nodes: Vec<Node>,
    pub flows: Vec<SequenceFlow>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum Node {
    StartEvent(StartEventNode),
    EndEvent(EndEventNode),
    ExclusiveGateway(ExclusiveGatewayNode),
    ServiceTask(ServiceTaskNode),
    ManualTask(ManualTaskNode),
    Unsupported(UnsupportedNode),
}

impl Node {
    pub fn id(&self) -> &str {
        match self {
            Node::StartEvent(n) => &n.id,
            Node::EndEvent(n) => &n.id,
            Node::ExclusiveGateway(n) => &n.id,
            Node::ServiceTask(n) => &n.id,
            Node::ManualTask(n) => &n.id,
            Node::Unsupported(n) => &n.id,
        }
    }
    pub fn element_type(&self) -> &str {
        match self {
            Node::StartEvent(_) => "startEvent",
            Node::EndEvent(_) => "endEvent",
            Node::ExclusiveGateway(_) => "exclusiveGateway",
            Node::ServiceTask(_) => "serviceTask",
            Node::ManualTask(_) => "manualTask",
            Node::Unsupported(n) => &n.element_type,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StartEventNode {
    pub id: String,
    pub name: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EndEventNode {
    pub id: String,
    pub name: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExclusiveGatewayNode {
    pub id: String,
    pub name: Option<String>,
    pub default_flow: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceTaskNode {
    pub id: String,
    pub name: Option<String>,
    pub task_type: String,
    pub retries: u32,
    pub io_mapping: Option<IoMapping>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ManualTaskNode {
    pub id: String,
    pub name: Option<String>,
    pub io_mapping: Option<IoMapping>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnsupportedNode {
    pub id: String,
    pub element_type: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SequenceFlow {
    pub id: String,
    pub source_ref: String,
    pub target_ref: String,
    pub condition: Option<String>,
    pub is_default: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct IoMapping {
    pub inputs: Vec<Mapping>,
    pub outputs: Vec<Mapping>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Mapping {
    pub source: String,
    pub target: String,
}
