use anyhow::{Context, Result};
use reqwest::Client;
use serde::de::DeserializeOwned;
use serde_json::Value;

#[derive(Clone)]
pub struct RestClient {
    client: Client,
    pub base_url: String,
}

impl RestClient {
    pub fn new(base_url: &str) -> Self {
        RestClient {
            client: Client::builder()
                .timeout(std::time::Duration::from_secs(30))
                .build()
                .expect("Failed to build HTTP client"),
            base_url: base_url.to_string(),
        }
    }

    pub async fn get<T: DeserializeOwned>(&self, path: &str) -> Result<T> {
        let url = format!("{}{}", self.base_url, path);
        let resp = self
            .client
            .get(&url)
            .send()
            .await
            .with_context(|| format!("GET {url}"))?;
        let status = resp.status();
        let body: T = resp
            .json()
            .await
            .with_context(|| format!("Parsing response from GET {url} ({status})"))?;
        Ok(body)
    }

    pub async fn get_raw(&self, path: &str) -> Result<(u16, Value)> {
        let url = format!("{}{}", self.base_url, path);
        let resp = self
            .client
            .get(&url)
            .send()
            .await
            .with_context(|| format!("GET {url}"))?;
        let status = resp.status().as_u16();
        let body: Value = resp
            .json()
            .await
            .with_context(|| format!("Parsing response from GET {url}"))?;
        Ok((status, body))
    }

    pub async fn post<B: serde::Serialize, T: DeserializeOwned>(
        &self,
        path: &str,
        body: &B,
    ) -> Result<T> {
        let url = format!("{}{}", self.base_url, path);
        let resp = self
            .client
            .post(&url)
            .json(body)
            .send()
            .await
            .with_context(|| format!("POST {url}"))?;
        let status = resp.status();
        let result: T = resp
            .json()
            .await
            .with_context(|| format!("Parsing response from POST {url} ({status})"))?;
        Ok(result)
    }

    /// Returns true if the daemon's /health endpoint returns 200.
    pub async fn is_healthy(&self) -> bool {
        matches!(self.get_raw("/health").await, Ok((200, _)))
    }
}
