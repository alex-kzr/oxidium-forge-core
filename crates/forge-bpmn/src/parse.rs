use quick_xml::events::{BytesStart, Event};
use quick_xml::name::ResolveResult;
use quick_xml::NsReader;

use crate::error::ParseError;
use crate::model::*;

const BPMN_NS: &[u8] = b"http://www.omg.org/spec/BPMN/20100524/MODEL";
const ZEEBE_NS: &[u8] = b"http://camunda.org/schema/zeebe/1.0";

/// Items pushed on the parser stack. Each variant holds the work-in-progress
/// data for the element currently being parsed.
enum StackItem {
    Root,
    Definitions,
    Process,
    StartEvent {
        id: String,
        name: Option<String>,
    },
    EndEvent {
        id: String,
        name: Option<String>,
    },
    ExclusiveGateway {
        id: String,
        name: Option<String>,
        default_flow: Option<String>,
    },
    ServiceTask {
        id: String,
        name: Option<String>,
        task_type: Option<String>,
        retries: u32,
        io_mapping: Option<IoMapping>,
    },
    ManualTask {
        id: String,
        name: Option<String>,
        io_mapping: Option<IoMapping>,
    },
    SequenceFlow {
        id: String,
        source: String,
        target: String,
        condition: Option<String>,
        is_default: bool,
    },
    Unsupported {
        id: String,
        element_type: String,
    },
    ExtensionElements,
    ZeebeIoMapping {
        inputs: Vec<Mapping>,
        outputs: Vec<Mapping>,
    },
    ConditionExpr {
        text: String,
    },
    /// Skip an element and all of its children (e.g. DI / diagram sections).
    Skip,
}

struct ParseContext {
    process_id: Option<String>,
    process_name: Option<String>,
    is_executable: bool,
    found_process: bool,
    nodes: Vec<Node>,
    flows: Vec<SequenceFlow>,
    stack: Vec<StackItem>,
}

/// Read an unqualified attribute by its local name.
fn get_attr(e: &BytesStart, name: &[u8]) -> Option<String> {
    e.attributes()
        .flatten()
        .find(|a| a.key.local_name().as_ref() == name)
        .map(|a| String::from_utf8_lossy(&a.value).into_owned())
}

pub fn parse_bpmn(xml: &[u8]) -> Result<ParsedProcess, ParseError> {
    let xml_str = std::str::from_utf8(xml)?;
    let mut reader = NsReader::from_str(xml_str);
    reader.trim_text(true);

    let mut ctx = ParseContext {
        process_id: None,
        process_name: None,
        is_executable: false,
        found_process: false,
        nodes: Vec::new(),
        flows: Vec::new(),
        stack: vec![StackItem::Root],
    };

    loop {
        match reader.read_resolved_event()? {
            (ns, Event::Start(e)) => {
                handle_start(&mut ctx, ns, &e, false)?;
            }
            (ns, Event::Empty(e)) => {
                handle_start(&mut ctx, ns, &e, true)?;
            }
            (_, Event::Text(t)) => {
                if let Some(StackItem::ConditionExpr { text }) = ctx.stack.last_mut() {
                    text.push_str(&t.unescape()?);
                }
            }
            (_, Event::End(_)) => {
                handle_end(&mut ctx);
            }
            (_, Event::Eof) => break,
            _ => {}
        }
    }

    if !ctx.found_process || !ctx.is_executable {
        return Err(ParseError::NoExecutableProcess);
    }
    let id = ctx.process_id.clone().unwrap_or_default();
    if id.is_empty() {
        return Err(ParseError::NoExecutableProcess);
    }

    Ok(ParsedProcess {
        id,
        name: ctx.process_name.clone(),
        executable: ctx.is_executable,
        nodes: ctx.nodes,
        flows: ctx.flows,
    })
}

fn is_bpmn(ns: &ResolveResult) -> bool {
    matches!(ns, ResolveResult::Bound(n) if n.as_ref() == BPMN_NS)
}
fn is_zeebe(ns: &ResolveResult) -> bool {
    matches!(ns, ResolveResult::Bound(n) if n.as_ref() == ZEEBE_NS)
}

fn handle_start(
    ctx: &mut ParseContext,
    ns: ResolveResult,
    e: &BytesStart,
    is_empty: bool,
) -> Result<(), ParseError> {
    let local = e.local_name();
    let local = local.as_ref();

    // If we're inside a Skip region, just keep nesting/skipping.
    if matches!(ctx.stack.last(), Some(StackItem::Skip)) {
        if !is_empty {
            ctx.stack.push(StackItem::Skip);
        }
        return Ok(());
    }

    // Zeebe extension elements.
    if is_zeebe(&ns) {
        match local {
            b"taskDefinition" => {
                let task_type = get_attr(e, b"type").unwrap_or_default();
                let retries = get_attr(e, b"retries")
                    .and_then(|s| s.parse::<u32>().ok())
                    .unwrap_or(3);
                apply_task_definition(ctx, task_type, retries);
                if !is_empty {
                    ctx.stack.push(StackItem::Skip);
                }
                return Ok(());
            }
            b"ioMapping" => {
                if is_empty {
                    // empty io mapping — nothing to collect
                } else {
                    ctx.stack.push(StackItem::ZeebeIoMapping {
                        inputs: Vec::new(),
                        outputs: Vec::new(),
                    });
                }
                return Ok(());
            }
            b"input" | b"output" => {
                let source = get_attr(e, b"source").unwrap_or_default();
                let target = get_attr(e, b"target").unwrap_or_default();
                if let Some(StackItem::ZeebeIoMapping { inputs, outputs }) = ctx.stack.last_mut() {
                    let mapping = Mapping { source, target };
                    if local == b"input" {
                        inputs.push(mapping);
                    } else {
                        outputs.push(mapping);
                    }
                }
                if !is_empty {
                    ctx.stack.push(StackItem::Skip);
                }
                return Ok(());
            }
            _ => {
                if !is_empty {
                    ctx.stack.push(StackItem::Skip);
                }
                return Ok(());
            }
        }
    }

    if is_bpmn(&ns) {
        match local {
            b"definitions" => {
                ctx.stack.push(StackItem::Definitions);
                return Ok(());
            }
            b"process" => {
                let id = get_attr(e, b"id").unwrap_or_default();
                let name = get_attr(e, b"name");
                let executable = get_attr(e, b"isExecutable")
                    .map(|v| v == "true")
                    .unwrap_or(false);
                // Only capture the first executable process.
                if executable && !ctx.found_process {
                    ctx.found_process = true;
                    ctx.is_executable = true;
                    ctx.process_id = Some(id);
                    ctx.process_name = name;
                    if is_empty {
                        // executable but empty process body
                    } else {
                        ctx.stack.push(StackItem::Process);
                    }
                } else {
                    // Non-executable or secondary process — skip its body.
                    if !is_empty {
                        ctx.stack.push(StackItem::Skip);
                    }
                }
                return Ok(());
            }
            b"BPMNDiagram" => {
                if !is_empty {
                    ctx.stack.push(StackItem::Skip);
                }
                return Ok(());
            }
            b"extensionElements" => {
                if !is_empty {
                    ctx.stack.push(StackItem::ExtensionElements);
                }
                return Ok(());
            }
            b"conditionExpression" => {
                if !is_empty {
                    ctx.stack.push(StackItem::ConditionExpr {
                        text: String::new(),
                    });
                }
                return Ok(());
            }
            b"incoming" | b"outgoing" => {
                // We rely on sourceRef/targetRef on flows; skip these.
                if !is_empty {
                    ctx.stack.push(StackItem::Skip);
                }
                return Ok(());
            }
            _ => {}
        }

        // Flow nodes / flows — only meaningful directly inside a process.
        let in_process = matches!(
            ctx.stack.last(),
            Some(StackItem::Process)
        );
        if in_process {
            let id = get_attr(e, b"id").unwrap_or_default();
            let name = get_attr(e, b"name");
            let item = match local {
                b"startEvent" => StackItem::StartEvent { id, name },
                b"endEvent" => StackItem::EndEvent { id, name },
                b"exclusiveGateway" => StackItem::ExclusiveGateway {
                    id,
                    name,
                    default_flow: get_attr(e, b"default"),
                },
                b"serviceTask" => StackItem::ServiceTask {
                    id,
                    name,
                    task_type: None,
                    retries: 3,
                    io_mapping: None,
                },
                b"manualTask" => StackItem::ManualTask {
                    id,
                    name,
                    io_mapping: None,
                },
                b"sequenceFlow" => StackItem::SequenceFlow {
                    id,
                    source: get_attr(e, b"sourceRef").unwrap_or_default(),
                    target: get_attr(e, b"targetRef").unwrap_or_default(),
                    condition: None,
                    is_default: false,
                },
                _ => StackItem::Unsupported {
                    id,
                    element_type: String::from_utf8_lossy(local).into_owned(),
                },
            };
            if is_empty {
                finish_item(ctx, item);
            } else {
                ctx.stack.push(item);
            }
            return Ok(());
        }
    }

    // Anything else: skip subtree.
    if !is_empty {
        ctx.stack.push(StackItem::Skip);
    }
    Ok(())
}

fn handle_end(ctx: &mut ParseContext) {
    if let Some(item) = ctx.stack.pop() {
        finish_item(ctx, item);
    }
}

/// Commit a popped (or empty) stack item into the accumulated model.
fn finish_item(ctx: &mut ParseContext, item: StackItem) {
    match item {
        StackItem::Root
        | StackItem::Definitions
        | StackItem::Process
        | StackItem::ExtensionElements
        | StackItem::Skip => {}
        StackItem::StartEvent { id, name } => {
            ctx.nodes.push(Node::StartEvent(StartEventNode { id, name }));
        }
        StackItem::EndEvent { id, name } => {
            ctx.nodes.push(Node::EndEvent(EndEventNode { id, name }));
        }
        StackItem::ExclusiveGateway {
            id,
            name,
            default_flow,
        } => {
            ctx.nodes.push(Node::ExclusiveGateway(ExclusiveGatewayNode {
                id,
                name,
                default_flow,
            }));
        }
        StackItem::ServiceTask {
            id,
            name,
            task_type,
            retries,
            io_mapping,
        } => {
            ctx.nodes.push(Node::ServiceTask(ServiceTaskNode {
                id,
                name,
                task_type: task_type.unwrap_or_default(),
                retries,
                io_mapping,
            }));
        }
        StackItem::ManualTask {
            id,
            name,
            io_mapping,
        } => {
            ctx.nodes.push(Node::ManualTask(ManualTaskNode {
                id,
                name,
                io_mapping,
            }));
        }
        StackItem::SequenceFlow {
            id,
            source,
            target,
            condition,
            mut is_default,
        } => {
            // A flow is the default of a gateway if a gateway lists it as `default`.
            // We mark is_default during the closing pass below; also derive from
            // gateway default_flow already-parsed nodes.
            if !is_default {
                is_default = ctx.nodes.iter().any(|n| {
                    if let Node::ExclusiveGateway(g) = n {
                        g.default_flow.as_deref() == Some(id.as_str())
                    } else {
                        false
                    }
                });
            }
            ctx.flows.push(SequenceFlow {
                id,
                source_ref: source,
                target_ref: target,
                condition,
                is_default,
            });
        }
        StackItem::Unsupported { id, element_type } => {
            ctx.nodes
                .push(Node::Unsupported(UnsupportedNode { id, element_type }));
        }
        StackItem::ConditionExpr { text } => {
            // Apply condition to the parent sequence flow.
            if let Some(StackItem::SequenceFlow { condition, .. }) = ctx.stack.last_mut() {
                let t = text.trim().to_string();
                if !t.is_empty() {
                    *condition = Some(t);
                }
            }
        }
        StackItem::ZeebeIoMapping { inputs, outputs } => {
            // Apply to nearest ServiceTask/ManualTask in the stack.
            apply_io_mapping(ctx, IoMapping { inputs, outputs });
        }
    }
}

fn apply_task_definition(ctx: &mut ParseContext, task_type: String, retries: u32) {
    for item in ctx.stack.iter_mut().rev() {
        if let StackItem::ServiceTask {
            task_type: tt,
            retries: r,
            ..
        } = item
        {
            *tt = Some(task_type);
            *r = retries;
            return;
        }
    }
}

fn apply_io_mapping(ctx: &mut ParseContext, mapping: IoMapping) {
    for item in ctx.stack.iter_mut().rev() {
        match item {
            StackItem::ServiceTask { io_mapping, .. } => {
                *io_mapping = Some(mapping);
                return;
            }
            StackItem::ManualTask { io_mapping, .. } => {
                *io_mapping = Some(mapping);
                return;
            }
            _ => {}
        }
    }
}
