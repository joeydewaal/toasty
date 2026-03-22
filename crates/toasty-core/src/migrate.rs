mod config;
mod history_file;
mod snapshot_file;

pub use config::{MigrationConfig, MigrationPrefixStyle};
pub use history_file::{HistoryFile, HistoryFileMigration};
pub use snapshot_file::SnapshotFile;

use serde::{Deserialize, Serialize};
use std::{fs, path::Path};

use crate::{Error, Result};

/// Top-level Toasty configuration, loaded from `Toasty.toml`.
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct Config {
    /// Migration-related configuration.
    #[serde(default)]
    pub migration: MigrationConfig,
}

impl Config {
    /// Create a new Config with default values
    pub fn new() -> Self {
        Self::default()
    }

    /// Load configuration from Toasty.toml in the project root
    pub fn load() -> Result<Self> {
        Self::from_path("Toasty.toml")
    }

    /// Load configuration from the given path
    pub fn from_path(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        let contents = fs::read_to_string(path).map_err(|e| {
            Error::migration_failed(format!("failed to read {}: {e}", path.display()))
        })?;

        toml::from_str(&contents).map_err(|e| {
            Error::migration_failed(format!("failed to parse {}: {e}", path.display()))
        })
    }
}
