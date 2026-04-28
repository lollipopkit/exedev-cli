use anyhow::{Context, Result};
use reqwest::StatusCode;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ExeDevApiError {
    #[error("exe.dev returned HTTP {status}: {body}")]
    Http { status: StatusCode, body: String },
}

impl ExeDevApiError {
    pub fn status(&self) -> StatusCode {
        match self {
            Self::Http { status, .. } => *status,
        }
    }

    pub fn body(&self) -> &str {
        match self {
            Self::Http { body, .. } => body,
        }
    }
}

pub struct ExeDevClient {
    endpoint: String,
    token: String,
    http: reqwest::Client,
}

impl ExeDevClient {
    pub fn new(endpoint: String, token: String) -> Self {
        Self {
            endpoint,
            token,
            http: reqwest::Client::new(),
        }
    }

    pub async fn exec(&self, command: &str) -> Result<String> {
        let response = self
            .http
            .post(&self.endpoint)
            .bearer_auth(&self.token)
            .body(command.to_string())
            .send()
            .await
            .context("request to exe.dev /exec failed")?;
        let status = response.status();
        let body = response
            .text()
            .await
            .context("failed to read exe.dev response body")?;
        if !status.is_success() {
            return Err(ExeDevApiError::Http { status, body }.into());
        }
        Ok(body)
    }
}
