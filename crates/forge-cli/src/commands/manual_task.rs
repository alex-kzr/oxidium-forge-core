use crate::client::RestClient;
use anyhow::{Context, Result};
use serde_json::Value;
use std::collections::HashMap;

pub async fn list(
    client: &RestClient,
    state_filter: Option<&str>,
    instance_key: Option<i64>,
    json: bool,
) -> Result<()> {
    let mut params = Vec::new();
    if let Some(s) = state_filter {
        params.push(format!("state={s}"));
    }
    if let Some(k) = instance_key {
        params.push(format!("instanceKey={k}"));
    }
    let path = if params.is_empty() {
        "/api/v1/manual-tasks".to_string()
    } else {
        format!("/api/v1/manual-tasks?{}", params.join("&"))
    };

    let (http_status, body) = client.get_raw(&path).await?;

    if http_status != 200 {
        eprintln!("Failed (HTTP {http_status}): {body}");
        std::process::exit(1);
    }

    if json {
        println!("{}", serde_json::to_string_pretty(&body)?);
        return Ok(());
    }

    let tasks = body
        .get("manualTasks")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();

    if tasks.is_empty() {
        println!("No manual tasks");
    } else {
        for t in &tasks {
            let key = t.get("key").and_then(|v| v.as_i64()).unwrap_or(0);
            let inst = t.get("instance_key").and_then(|v| v.as_i64()).unwrap_or(0);
            let st = t.get("state").and_then(|v| v.as_str()).unwrap_or("?");
            let elem = t.get("element_id").and_then(|v| v.as_str()).unwrap_or("?");
            println!("ManualTask key={key} [{st}] element={elem} instance={inst}");
        }
    }

    Ok(())
}

pub async fn complete(
    client: &RestClient,
    task_key: i64,
    vars: &[(String, String)],
    variables_file: Option<&std::path::Path>,
    json: bool,
) -> Result<()> {
    let mut variables: HashMap<String, Value> = HashMap::new();

    if let Some(path) = variables_file {
        let content = tokio::fs::read_to_string(path)
            .await
            .with_context(|| format!("Reading variables file {:?}", path))?;
        let parsed: HashMap<String, Value> =
            serde_json::from_str(&content).context("Parsing variables JSON file")?;
        variables.extend(parsed);
    }

    for kv in vars {
        let (k, v) = kv.0.split_once('=').with_context(|| {
            format!("--var argument '{}' must be in key=value form", kv.0)
        })?;
        let parsed: Value =
            serde_json::from_str(v).unwrap_or_else(|_| Value::String(v.to_string()));
        variables.insert(k.to_string(), parsed);
    }

    let body = serde_json::json!({"variables": variables});
    let url = format!(
        "{}/api/v1/manual-tasks/{task_key}/completion",
        client.base_url
    );
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
    if status.is_success() {
        if json {
            let body_val: Value = resp.json().await.context("Parsing response")?;
            println!("{}", serde_json::to_string_pretty(&body_val)?);
        } else {
            println!("ManualTask {task_key} completed");
        }
    } else {
        let body_val: Value = resp.json().await.unwrap_or(Value::Null);
        eprintln!("Failed (HTTP {status}): {body_val}");
        std::process::exit(1);
    }

    Ok(())
}

pub async fn cancel(
    client: &RestClient,
    task_key: i64,
    reason: Option<&str>,
    json: bool,
) -> Result<()> {
    let body = serde_json::json!({"reason": reason.unwrap_or("")});
    let url = format!(
        "{}/api/v1/manual-tasks/{task_key}/cancellation",
        client.base_url
    );
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
    if status.is_success() {
        if !json {
            println!("ManualTask {task_key} cancelled");
        }
    } else {
        let body_val: Value = resp.json().await.unwrap_or(Value::Null);
        eprintln!("Failed (HTTP {status}): {body_val}");
        std::process::exit(1);
    }

    Ok(())
}
