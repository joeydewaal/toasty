use crate::{Error, Result};
use serde::{Deserialize, Serialize};
use std::fmt;
use std::path::{Path, PathBuf};
use std::str::FromStr;

const HISTORY_FILE_VERSION: u32 = 1;

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

/// Configuration for migration operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MigrationConfig {
    /// Path to the migrations folder
    pub path: PathBuf,

    /// Style of migration file prefixes
    pub prefix_style: MigrationPrefixStyle,

    /// Whether the history file should store and verify checksums of the migration files so that
    /// they may not be changed.
    pub checksums: bool,

    /// Whether to add statement breakpoint comments to generated SQL migration files.
    /// These comments mark boundaries where SQL statements should be split for execution.
    /// This is needed because different databases have different batching capabilities:
    /// some (like PostgreSQL) can execute multiple statements in one batch, while others
    /// require each statement to be executed separately.
    pub statement_breakpoints: bool,
}

/// Style for migration file name prefixes
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MigrationPrefixStyle {
    /// Sequential numbering (e.g., 0001_, 0002_, 0003_)
    Sequential,

    /// Timestamp-based (e.g., 20240112_153045_)
    Timestamp,
}

impl Default for MigrationConfig {
    fn default() -> Self {
        Self {
            path: PathBuf::from("toasty"),
            prefix_style: MigrationPrefixStyle::Sequential,
            checksums: false,
            statement_breakpoints: true,
        }
    }
}

impl MigrationConfig {
    /// Create a new MigrationConfig with default values
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the migrations path
    pub fn path(mut self, path: impl Into<PathBuf>) -> Self {
        self.path = path.into();
        self
    }

    /// Set the migration prefix style
    pub fn prefix_style(mut self, style: MigrationPrefixStyle) -> Self {
        self.prefix_style = style;
        self
    }

    /// Returns the directory of the migration files derived from `path`.
    pub fn get_migrations_dir(&self) -> PathBuf {
        self.path.join("migrations")
    }

    /// Returns the directory of the snapshot files derived from `path`.
    pub fn get_snapshots_dir(&self) -> PathBuf {
        self.path.join("snapshots")
    }

    /// Get the path to the history file
    pub fn get_history_file_path(&self) -> PathBuf {
        self.path.join("history.toml")
    }
}

/// History file containing the record of all applied migrations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoryFile {
    /// History file format version
    version: u32,

    /// Migration history
    migrations: Vec<HistoryFileMigration>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoryFileMigration {
    /// Random unique identifier for this migration.
    pub id: u64,

    /// Migration name/identifier.
    pub name: String,

    /// Name of the snapshot generated alongside this migration.
    pub snapshot_name: String,

    /// Optional checksum of the migration file to detect changes
    #[serde(skip_serializing_if = "Option::is_none")]
    pub checksum: Option<String>,
}

impl HistoryFile {
    /// Create a new empty history file
    pub fn new() -> Self {
        Self {
            version: HISTORY_FILE_VERSION,
            migrations: Vec::new(),
        }
    }

    /// Load a history file from a TOML file
    pub fn load(path: impl AsRef<Path>) -> Result<Self> {
        let contents = std::fs::read_to_string(path.as_ref())
            .map_err(|e| Error::migration_failed(e.to_string()))?;
        contents.parse()
    }

    /// Save the history file to a TOML file
    pub fn save(&self, path: impl AsRef<Path>) -> Result<()> {
        std::fs::write(path.as_ref(), self.to_string())
            .map_err(|e| Error::migration_failed(e.to_string()))?;
        Ok(())
    }

    /// Loads the history file, or returns an empty one if it does not exist
    pub fn load_or_default(path: impl AsRef<Path>) -> Result<Self> {
        if std::fs::exists(&path).map_err(|e| Error::migration_failed(e.to_string()))? {
            return Self::load(path);
        }
        Ok(Self::default())
    }

    pub fn migrations(&self) -> &[HistoryFileMigration] {
        &self.migrations
    }

    /// Get the next migration number by parsing the last migration's name
    pub fn next_migration_number(&self) -> u32 {
        self.migrations
            .last()
            .and_then(|m| {
                // Extract the first 4 digits from the migration name (e.g., "0001_migration.sql" -> 1)
                m.name.split('_').next()?.parse::<u32>().ok()
            })
            .map(|n| n + 1)
            .unwrap_or(0)
    }

    /// Add a migration to the history
    pub fn add_migration(&mut self, migration: HistoryFileMigration) {
        self.migrations.push(migration);
    }

    /// Remove a migration from the history by index
    pub fn remove_migration(&mut self, index: usize) {
        self.migrations.remove(index);
    }
}

impl Default for HistoryFile {
    fn default() -> Self {
        Self::new()
    }
}

impl FromStr for HistoryFile {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self> {
        let file: HistoryFile =
            toml::from_str(s).map_err(|e| Error::migration_failed(e.to_string()))?;

        if file.version != HISTORY_FILE_VERSION {
            return Err(Error::migration_failed(format!(
                "Unsupported history file version: {}. Expected version {}",
                file.version, HISTORY_FILE_VERSION
            )));
        }

        Ok(file)
    }
}

impl fmt::Display for HistoryFile {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let toml_str = toml::to_string_pretty(self).map_err(|_| fmt::Error)?;
        write!(f, "{}", toml_str)
    }
}
