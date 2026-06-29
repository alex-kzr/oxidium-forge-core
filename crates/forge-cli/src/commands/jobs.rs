use crate::client::RestClient;
use anyhow::{Context, Result};
use serde_json::Value;
use std::collections::HashMap;

pub async fn activate(
    client: &RestClient,
    task_type: &str,
    worker: &str,
    max_jobs: i64,
    lock_duration: i64,
    json: bool,
) -> Result<()> {
    let body = serde_json::json!({
        "taskType": task_type,
        "worker": worker,
        "maxJobs": max_jobs,
        "lockDuration": lock_duration,
    });

    let url = format!("{}/api/v1/jobs/activation", client.base_url);
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
            let jobs = body_val
                .get("jobs")
                .and_then(|v| v.as_array())
                .cloned()
                .unwrap_or_default();
            if jobs.is_empty() {
                println!("No jobs available for type '{task_type}'");
            } else {
                for j in &jobs {
                    let key = j.get("key").and_then(|v| v.as_i64()).unwrap_or(0);
                    let instance = j
                        .get("instance_key")
                        .and_then(|v| v.as_i64())
                        .unwrap_or(0);
                    let retries = j.get("retries").and_then(|v| v.as_i64()).unwrap_or(0);
                    println!("Job key={key} instance={instance} retries={retries}");
                    if let Some(vars) = j.get("variables").and_then(|v| v.as_object()) {
                        for (k, v) in vars {
                            println!("  {k} = {v}");
                        }
                    }
                }
            }
        }
    } else {
        eprintln!("Failed (HTTP {status}): {body_val}");
        std::process::exit(1);
    }

    Ok(())
}

pub async fn complete(
    client: &RestClient,
    job_key: i64,
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
    let url = format!("{}/api/v1/jobs/{job_key}/completion", client.base_url);
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
            println!("Job {job_key} completed");
        }
    } else {
        let body_val: Value = resp.json().await.unwrap_or(Value::Null);
        eprintln!("Failed (HTTP {status}): {body_val}");
        std::process::exit(1);
    }

    Ok(())
}

pub async fn fail(
    client: &RestClient,
    job_key: i64,
    error_message: &str,
    retries: i64,
    retry_backoff: Option<i64>,
    json: bool,
) -> Result<()> {
    let mut body = serde_json::json!({
        "errorMessage": error_message,
        "retries": retries,
    });
    if let Some(backoff) = retry_backoff {
        body["retryBackoff"] = serde_json::json!(backoff);
    }

    let url = format!("{}/api/v1/jobs/{job_key}/failure", client.base_url);
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
            println!("Job {job_key} failed (retries={retries})");
        }
    } else {
        let body_val: Value = resp.json().await.unwrap_or(Value::Null);
        eprintln!("Failed (HTTP {status}): {body_val}");
        std::process::exit(1);
    }

    Ok(())
}
