use crate::client::RestClient;
use anyhow::Result;

pub async fn list(client: &RestClient, json: bool) -> Result<()> {
    let body: serde_json::Value = client.get("/api/v1/process-definitions").await?;
    if json {
        println!("{}", serde_json::to_string_pretty(&body)?);
        return Ok(());
    }
    let defs = body.get("process_definitions").and_then(|d| d.as_array());
    match defs {
        Some(defs) if !defs.is_empty() => {
            println!(
                "{:<20} {:>7}  {:>6}  {:<8}  {}",
                "PROCESS ID", "VERSION", "KEY", "ACTIVE", "RESOURCE"
            );
            println!("{}", "-".repeat(80));
            for d in defs {
                let pid = d.get("bpmn_process_id").and_then(|v| v.as_str()).unwrap_or("?");
                let ver = d.get("version").and_then(|v| v.as_i64()).unwrap_or(0);
                let key = d.get("key").and_then(|v| v.as_i64()).unwrap_or(0);
                let active = d.get("is_active").and_then(|v| v.as_bool()).unwrap_or(false);
                let res = d.get("resource_name").and_then(|v| v.as_str()).unwrap_or("?");
                let active_str = if active { "yes" } else { "no" };
                println!(
                    "{:<20} {:>7}  {:>6}  {:<8}  {}",
                    pid, ver, key, active_str, res
                );
            }
        }
        _ => println!("No process definitions found."),
    }
    Ok(())
}

pub async fn activate(client: &RestClient, key: i64, json: bool) -> Result<()> {
    let url = format!("/api/v1/process-definitions/{key}/activation");
    let body: serde_json::Value = client.post(&url, &serde_json::json!({})).await?;
    if json {
        println!("{}", serde_json::to_string_pretty(&body)?);
    } else {
        println!("Activated definition key={key}");
    }
    Ok(())
}
