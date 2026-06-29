use crate::client::RestClient;
use anyhow::{Context, Result};
use serde_json::Value;

pub async fn list(client: &RestClient, state_filter: Option<&str>, json: bool) -> Result<()> {
    let path = match state_filter {
        Some(s) => format!("/api/v1/incidents?state={s}"),
        None => "/api/v1/incidents".to_string(),
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

    let incidents = body
        .get("incidents")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();

    if incidents.is_empty() {
        println!("No incidents");
    } else {
        for inc in &incidents {
            let key = inc.get("key").and_then(|v| v.as_i64()).unwrap_or(0);
            let itype = inc
                .get("incident_type")
                .and_then(|v| v.as_str())
                .unwrap_or("?");
            let istate = inc.get("state").and_then(|v| v.as_str()).unwrap_or("?");
            let msg = inc.get("message").and_then(|v| v.as_str()).unwrap_or("");
            let inst = inc
                .get("instance_key")
                .and_then(|v| v.as_i64())
                .unwrap_or(0);
            println!("Incident key={key} [{istate}] type={itype} instance={inst}");
            if !msg.is_empty() {
                println!("  {msg}");
            }
        }
    }

    Ok(())
}

pub async fn resolve(client: &RestClient, incident_key: i64, json: bool) -> Result<()> {
    let url = format!(
        "{}/api/v1/incidents/{incident_key}/resolution",
        client.base_url
    );
    let http = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()?;
    let resp = http
        .post(&url)
        .json(&serde_json::json!({}))
        .send()
        .await
        .with_context(|| format!("POST {url}"))?;

    let status = resp.status();
    if status.is_success() {
        if !json {
            println!("Incident {incident_key} resolved");
        }
    } else {
        let body_val: Value = resp.json().await.unwrap_or(Value::Null);
        eprintln!("Failed (HTTP {status}): {body_val}");
        std::process::exit(1);
    }

    Ok(())
}
