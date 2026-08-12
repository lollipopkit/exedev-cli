use anyhow::{Context, Result, bail};
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
        // Every request carries the API key as a bearer token, so the endpoint has
        // to be HTTPS: `--endpoint http://elsewhere/collect` would otherwise send
        // the key and the command in the clear to whatever the caller named.
        if !self.endpoint.to_ascii_lowercase().starts_with("https://") {
            bail!(
                "endpoint must be an https:// URL to carry the API key, got {}",
                self.endpoint
            );
        }
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
