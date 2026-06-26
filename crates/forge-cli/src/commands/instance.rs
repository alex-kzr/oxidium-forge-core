use crate::client::RestClient;
use anyhow::{Context, Result};
use serde_json::Value;
use std::collections::HashMap;

pub async fn start(
    client: &RestClient,
    bpmn_process_id: &str,
    vars: &[(String, String)],
    variables_file: Option<&std::path::Path>,
    json: bool,
) -> Result<()> {
    let mut variables: HashMap<String, Value> = HashMap::new();

    // Load from file if provided.
    if let Some(path) = variables_file {
        let content = tokio::fs::read_to_string(path)
            .await
            .with_context(|| format!("Reading variables file {:?}", path))?;
        let parsed: HashMap<String, Value> =
            serde_json::from_str(&content).context("Parsing variables JSON file")?;
        variables.extend(parsed);
    }

    // Apply --var k=v pairs (override file).
    for kv in vars {
        let (k, v) = kv.0.split_once('=').with_context(|| {
            format!("--var argument '{}' must be in key=value form", kv.0)
        })?;
        let parsed: Value = serde_json::from_str(v)
            .unwrap_or_else(|_| Value::String(v.to_string()));
        variables.insert(k.to_string(), parsed);
    }

    let body = serde_json::json!({
        "bpmnProcessId": bpmn_process_id,
        "variables": variables,
    });

    let url = format!("{}/api/v1/process-instances", client.base_url);
    let http = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()?;
    let resp = http
        .post(&url)
        .json(&body)
        .send()
        .await
        .with_context(|| format!("POST {url}"))?;

    let status = resp.status();
    let body_val: Value = resp.json().await.context("Parsing response")?;

    if status.is_success() {
        if json {
            println!("{}", serde_json::to_string_pretty(&body_val)?);
        } else {
            let key = body_val.get("key").and_then(|v| v.as_i64()).unwrap_or(0);
            let st = body_val
                .get("status")
                .and_then(|v| v.as_str())
                .unwrap_or("?");
            println!("Started instance key={key} status={st}");
        }
    } else {
        if json {
            eprintln!("{}", serde_json::to_string_pretty(&body_val)?);
        } else {
            eprintln!("Failed (HTTP {status}): {body_val}");
        }
        std::process::exit(1);
    }

    Ok(())
}

pub async fn status(client: &RestClient, key: i64, json: bool) -> Result<()> {
    let (http_status, body) = client
        .get_raw(&format!("/api/v1/process-instances/{key}"))
        .await?;

    if http_status == 404 {
        eprintln!("Instance {key} not found");
        std::process::exit(1);
    }

    if json {
        println!("{}", serde_json::to_string_pretty(&body)?);
        return Ok(());
    }

    let st = body.get("status").and_then(|v| v.as_str()).unwrap_or("?");
    let pid = body
        .get("bpmn_process_id")
        .and_then(|v| v.as_str())
        .unwrap_or("?");
    let ver = body.get("version").and_then(|v| v.as_i64()).unwrap_or(0);
    println!("Instance {key}: {pid} v{ver} — {st}");

    if let Some(elements) = body.get("active_elements").and_then(|v| v.as_array()) {
        if !elements.is_empty() {
            println!("  Active elements:");
            for el in elements {
                let eid = el.get("element_id").and_then(|v| v.as_str()).unwrap_or("?");
                let etype = el.get("element_type").and_then(|v| v.as_str()).unwrap_or("?");
                println!("    {eid} ({etype})");
            }
        }
    }

    if let Some(vars) = body.get("variables").and_then(|v| v.as_object()) {
        if !vars.is_empty() {
            println!("  Variables:");
            for (k, v) in vars {
                println!("    {k} = {v}");
            }
        }
    }

    Ok(())
}
