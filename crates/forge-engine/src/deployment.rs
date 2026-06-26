use forge_bpmn::{compile, parse_bpmn, validate};
use forge_store::{definitions::DefinitionRepo, Store};
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeployedDefinition {
    pub key: i64,
    pub bpmn_process_id: String,
    pub version: i64,
    pub resource_name: String,
    pub is_active: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DeployDiagnostic {
    pub element_id: Option<String>,
    pub code: String,
    pub message: String,
}

#[derive(Debug, Error)]
pub enum DeployError {
    #[error("Parse error: {0}")]
    Parse(#[from] forge_bpmn::ParseError),
    #[error("Validation failed with {count} error(s)")]
    Validation {
        count: usize,
        diagnostics: Vec<DeployDiagnostic>,
    },
    #[error("Store error: {0}")]
    Store(#[from] forge_model::StoreError),
    #[error("Serialization error: {0}")]
    Json(#[from] serde_json::Error),
}

pub async fn deploy(
    store: &Store,
    resource_name: &str,
    xml_bytes: &[u8],
    auto_activate: bool,
) -> Result<DeployedDefinition, DeployError> {
    // 1. Parse
    let xml_str = std::str::from_utf8(xml_bytes).map_err(forge_bpmn::ParseError::Utf8)?;
    let parsed = parse_bpmn(xml_bytes)?;

    // 2. Validate
    let diagnostics = validate(&parsed);
    if !diagnostics.is_empty() {
        return Err(DeployError::Validation {
            count: diagnostics.len(),
            diagnostics: diagnostics
                .into_iter()
                .map(|d| DeployDiagnostic {
                    element_id: d.element_id,
                    code: d.code,
                    message: d.message,
                })
                .collect(),
        });
    }

    // 3. Compile
    let graph = compile(&parsed);
    let graph_json = serde_json::to_string(&graph)?;

    // 4. Compute checksum
    let checksum = compute_checksum(xml_bytes);

    // 5. Persist
    let repo = DefinitionRepo::new(&store.pool);
    let row = repo
        .insert_new_version(
            &parsed.id,
            resource_name,
            xml_str,
            &graph_json,
            &checksum,
            auto_activate,
        )
        .await?;

    Ok(DeployedDefinition {
        key: row.key,
        bpmn_process_id: row.bpmn_process_id,
        version: row.version,
        resource_name: row.resource_name,
        is_active: row.is_active != 0,
    })
}

fn compute_checksum(data: &[u8]) -> String {
    let mut hash: u64 = 14695981039346656037;
    for byte in data {
        hash ^= *byte as u64;
        hash = hash.wrapping_mul(1099511628211);
    }
    format!("{:016x}", hash)
}
