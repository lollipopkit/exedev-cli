use anyhow::{Context, Result};
use reqwest::StatusCode;
use thiserror::Error;

#[derive(Debug, Error)]
enum ApiError {
    #[error("exe.dev returned HTTP {status}: {body}")]
    Http { status: StatusCode, body: String },
}

pub(crate) struct ExeDevClient {
    endpoint: String,
    token: String,
    http: reqwest::Client,
}

impl ExeDevClient {
    pub(crate) fn new(endpoint: String, token: String) -> Self {
        Self {
            endpoint,
            token,
            http: reqwest::Client::new(),
        }
    }

    pub(crate) async fn exec(&self, command: &str) -> Result<String> {
        let response = self
            .http
            .post(&self.endpoint)
            .bearer_auth(&self.token)
            .body(command.to_string())
            .send()
            .await
            .context("request to exe.dev /exec failed")?;
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        if !status.is_success() {
            return Err(ApiError::Http { status, body }.into());
        }
        Ok(body)
    }
}
