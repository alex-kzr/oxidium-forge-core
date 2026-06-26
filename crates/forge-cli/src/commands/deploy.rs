use crate::client::RestClient;
use anyhow::{Context, Result};
use std::path::Path;

pub async fn validate_bpmn(client: &RestClient, file: &Path, json: bool) -> Result<()> {
    let bytes = tokio::fs::read(file)
        .await
        .with_context(|| format!("Reading {:?}", file))?;
    let filename = file
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("upload.bpmn");

    // Use ?activate=false as a dry-run: deploy without activating, inspect errors.
    let url = format!("{}/api/v1/deployments?activate=false", client.base_url);
    let part = reqwest::multipart::Part::bytes(bytes.clone())
        .file_name(filename.to_string())
        .mime_str("application/xml")
        .context("Building multipart")?;
    let form = reqwest::multipart::Form::new().part("file", part);

    let http = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()?;
    let resp = http
        .post(&url)
        .multipart(form)
        .send()
        .await
        .with_context(|| format!("POST {url}"))?;

    let status = resp.status();
    let body: serde_json::Value = resp.json().await.context("Parsing response")?;

    // A successful HTTP status with an empty `errors` array means valid.
    let has_errors = body
        .get("errors")
        .and_then(|e| e.as_array())
        .map(|a| !a.is_empty())
        .unwrap_or(false);

    if status.is_success() && !has_errors {
        if json {
            println!("{}", serde_json::to_string_pretty(&body)?);
        } else {
            println!("OK BPMN is valid: {:?}", filename);
        }
        Ok(())
    } else {
        if json {
            println!("{}", serde_json::to_string_pretty(&body)?);
        } else {
            eprintln!("FAILED Validation failed for {:?}:", filename);
            if let Some(errors) = body.get("errors").and_then(|e| e.as_array()) {
                for err in errors {
                    if let Some(diags) = err.get("diagnostics").and_then(|d| d.as_array()) {
                        for d in diags {
                            let code = d.get("code").and_then(|c| c.as_str()).unwrap_or("ERROR");
                            let msg = d.get("message").and_then(|m| m.as_str()).unwrap_or("");
                            let id = d.get("element_id").and_then(|i| i.as_str()).unwrap_or("-");
                            eprintln!("  [{code}] {id}: {msg}");
                        }
                    } else {
                        eprintln!("  {}", err);
                    }
                }
            }
        }
        std::process::exit(1);
    }
}

pub async fn deploy_bpmn(client: &RestClient, file: &Path, activate: bool, json: bool) -> Result<()> {
    let bytes = tokio::fs::read(file)
        .await
        .with_context(|| format!("Reading {:?}", file))?;
    let filename = file
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("upload.bpmn");

    let url = format!("{}/api/v1/deployments?activate={}", client.base_url, activate);
    let part = reqwest::multipart::Part::bytes(bytes)
        .file_name(filename.to_string())
        .mime_str("application/xml")
        .context("Building multipart")?;
    let form = reqwest::multipart::Form::new().part("file", part);

    let http = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()?;
    let resp = http
        .post(&url)
        .multipart(form)
        .send()
        .await
        .with_context(|| format!("POST {url}"))?;

    let status = resp.status();
    let body: serde_json::Value = resp.json().await.context("Parsing response")?;

    let has_errors = body
        .get("errors")
        .and_then(|e| e.as_array())
        .map(|a| !a.is_empty())
        .unwrap_or(false);

    if status.is_success() && !has_errors {
        if json {
            println!("{}", serde_json::to_string_pretty(&body)?);
        } else if let Some(deps) = body.get("deployments").and_then(|d| d.as_array()) {
            for dep in deps {
                let pid = dep.get("bpmn_process_id").and_then(|v| v.as_str()).unwrap_or("?");
                let ver = dep.get("version").and_then(|v| v.as_i64()).unwrap_or(0);
                let key = dep.get("key").and_then(|v| v.as_i64()).unwrap_or(0);
                let active = dep.get("is_active").and_then(|v| v.as_bool()).unwrap_or(false);
                println!("Deployed: {pid} v{ver} (key={key}, active={active})");
            }
        }
        Ok(())
    } else {
        if json {
            eprintln!("{}", serde_json::to_string_pretty(&body)?);
        } else {
            eprintln!("Deploy failed (HTTP {status}): {body}");
        }
        std::process::exit(1);
    }
}
