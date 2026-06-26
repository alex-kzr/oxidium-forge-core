use anyhow::Result;

use crate::client::RestClient;

pub async fn run(client: &RestClient, json_output: bool) -> Result<()> {
    let (status, body) = client.get_raw("/health").await?;

    if json_output {
        println!("{}", serde_json::to_string_pretty(&body)?);
    } else {
        let state = body
            .get("status")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown");
        let db = body
            .get("db")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown");
        let version = body
            .get("version")
            .and_then(|v| v.as_str())
            .unwrap_or("?");

        if status == 200 {
            println!("✓ Daemon healthy (v{version})");
            println!("  Database: {db}");
        } else {
            println!("✗ Daemon degraded (HTTP {status})");
            println!("  Status: {state}");
        }
    }

    Ok(())
}
