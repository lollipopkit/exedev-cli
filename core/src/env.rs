use anyhow::{Context, Result};
use std::{io::ErrorKind, path::PathBuf};

/// Load the nearest `.env` file without overriding existing environment values.
pub fn load_dotenv() -> Result<Option<PathBuf>> {
    match dotenvy::dotenv() {
        Ok(path) => Ok(Some(path)),
        Err(dotenvy::Error::Io(err)) if err.kind() == ErrorKind::NotFound => Ok(None),
        Err(err) => Err(err).context("failed to load .env"),
    }
}
