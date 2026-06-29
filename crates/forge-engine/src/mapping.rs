use crate::error::EngineError;
use crate::expr::{eval, parse};
use forge_bpmn::graph::CompiledIoMapping;
use serde_json::Value;
use std::collections::HashMap;

/// Evaluate all input mappings against `scope_vars` and return the job payload.
/// If no mapping is defined, returns a shallow clone of the full scope.
pub fn apply_inputs(
    scope_vars: &HashMap<String, Value>,
    mapping: Option<&CompiledIoMapping>,
) -> Result<HashMap<String, Value>, EngineError> {
    let Some(m) = mapping else {
        return Ok(scope_vars.clone());
    };
    if m.inputs.is_empty() {
        return Ok(scope_vars.clone());
    }

    let mut out = HashMap::new();
    for entry in &m.inputs {
        let expr = parse(&entry.source)
            .map_err(|e| EngineError::Expression(format!("input mapping parse: {e}")))?;
        let val = eval(&expr, scope_vars)
            .map_err(|e| EngineError::Expression(format!("input mapping eval: {e}")))?;
        out.insert(entry.target.clone(), val);
    }
    Ok(out)
}

/// Evaluate all output mappings against `result_vars` and merge into `scope_vars`.
/// If no mapping is defined, merges the full result into scope (shallow).
pub fn apply_outputs(
    scope_vars: &mut HashMap<String, Value>,
    result_vars: &HashMap<String, Value>,
    mapping: Option<&CompiledIoMapping>,
) -> Result<(), EngineError> {
    let Some(m) = mapping else {
        scope_vars.extend(result_vars.clone());
        return Ok(());
    };
    if m.outputs.is_empty() {
        scope_vars.extend(result_vars.clone());
        return Ok(());
    }

    for entry in &m.outputs {
        let expr = parse(&entry.source)
            .map_err(|e| EngineError::Expression(format!("output mapping parse: {e}")))?;
        let val = eval(&expr, result_vars)
            .map_err(|e| EngineError::Expression(format!("output mapping eval: {e}")))?;
        scope_vars.insert(entry.target.clone(), val);
    }
    Ok(())
}
