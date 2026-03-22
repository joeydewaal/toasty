mod config;
mod history_file;
mod snapshot_file;

pub use config::{MigrationConfig, MigrationPrefixStyle};
pub use history_file::{HistoryFile, HistoryFileMigration};
pub use snapshot_file::SnapshotFile;

use serde::{Deserialize, Serialize};
use std::path::Path;

use crate::{Error, Result};

/// Top-level Toasty configuration, loaded from `Toasty.toml`.
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct Config {
    /// Migration-related configuration.
    #[serde(default)]
    pub migration: MigrationConfig,
}

impl Config {
    /// Load configuration from `Toasty.toml` in the current working directory.
    pub fn load() -> Result<Self> {
        Self::load_or_default("Toasty.toml")
    }

    /// Load configuration from the given path, returning a default if the file
    /// does not exist.
    pub fn load_or_default(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        if !path.exists() {
            return Ok(Self::default());
        }
        let contents = std::fs::read_to_string(path).map_err(|e| {
            Error::migration_failed(format!("failed to read {}: {e}", path.display()))
        })?;
        toml::from_str(&contents).map_err(|e| {
            Error::migration_failed(format!("failed to parse {}: {e}", path.display()))
        })
    }
}
