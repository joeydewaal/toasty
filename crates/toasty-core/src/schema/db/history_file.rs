const HISTORY_FILE_VERSION: u32 = 1;

/// A TOML-serializable record of all migrations that have been generated.
///
/// The history file lives at `<migration_path>/history.toml` and is the
/// source of truth for which migrations exist and what order they were
/// created in. Each entry is a [`HistoryFileMigration`].
///
/// The file carries a version number so that the format can evolve without
/// silently misinterpreting older files.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct HistoryFile {
    /// History file format version.
    version: u32,

    /// Ordered list of migrations.
    #[cfg_attr(feature = "serde", serde(default))]
    migrations: Vec<HistoryFileMigration>,
}

/// A single entry in the migration history.
///
/// Each entry records the randomly-assigned ID used by the database driver to
/// track application status, the migration SQL file name, the companion
/// snapshot file name, and an optional checksum.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct HistoryFileMigration {
    /// Random unique identifier for this migration.
    pub id: u64,

    /// Migration file name (e.g. `"0001_create_users.sql"`).
    pub name: String,

    /// Name of the snapshot generated alongside this migration.
    pub snapshot_name: String,

    /// Optional checksum of the migration file to detect changes.
    #[cfg_attr(
        feature = "serde",
        serde(default, skip_serializing_if = "Option::is_none")
    )]
    pub checksum: Option<String>,
}

impl HistoryFile {
    /// Creates a new empty history file with the current format version.
    pub fn new() -> Self {
        Self {
            version: HISTORY_FILE_VERSION,
            migrations: Vec::new(),
        }
    }

    /// Returns the file format version.
    pub fn version(&self) -> u32 {
        self.version
    }

    /// Returns the ordered list of migrations in this history.
    pub fn migrations(&self) -> &[HistoryFileMigration] {
        &self.migrations
    }

    /// Gets the next migration number by parsing the last migration's name.
    ///
    /// Migration names are expected to start with a zero-padded number
    /// (e.g. `"0001_create_users.sql"`). Returns `0` if the history is empty
    /// or the last name cannot be parsed.
    pub fn next_migration_number(&self) -> u32 {
        self.migrations
            .last()
            .and_then(|m| m.name.split('_').next()?.parse::<u32>().ok())
            .map(|n| n + 1)
            .unwrap_or(0)
    }

    /// Appends a migration to the history.
    pub fn add_migration(&mut self, migration: HistoryFileMigration) {
        self.migrations.push(migration);
    }

    /// Removes a migration from the history by index.
    pub fn remove_migration(&mut self, index: usize) {
        self.migrations.remove(index);
    }

    /// Validates that the history file version matches the expected format.
    ///
    /// Returns `true` if the version is supported, `false` otherwise.
    pub fn is_supported_version(&self) -> bool {
        self.version == HISTORY_FILE_VERSION
    }
}

#[cfg(feature = "toml")]
impl HistoryFile {
    /// Loads a history file from a TOML file at the given path.
    ///
    /// Returns an error if the file cannot be read, parsed, or has an
    /// unsupported version number.
    pub fn load(path: impl AsRef<std::path::Path>) -> Result<Self, HistoryFileError> {
        let contents = std::fs::read_to_string(path.as_ref())?;
        let file: Self = toml::from_str(&contents)?;

        if !file.is_supported_version() {
            return Err(HistoryFileError::UnsupportedVersion(file.version));
        }

        Ok(file)
    }

    /// Saves the history file as TOML to the given path.
    pub fn save(&self, path: impl AsRef<std::path::Path>) -> Result<(), HistoryFileError> {
        let toml_str = toml::to_string_pretty(self)?;
        std::fs::write(path.as_ref(), toml_str)?;
        Ok(())
    }

    /// Loads a history file from the given path, returning an empty default
    /// if the file does not exist.
    pub fn load_or_default(path: impl AsRef<std::path::Path>) -> Result<Self, HistoryFileError> {
        if path.as_ref().exists() {
            return Self::load(path);
        }
        Ok(Self::default())
    }
}

impl Default for HistoryFile {
    fn default() -> Self {
        Self::new()
    }
}

/// Errors that can occur when loading or saving a [`HistoryFile`].
#[cfg(feature = "toml")]
#[derive(Debug)]
pub enum HistoryFileError {
    /// An I/O error occurred reading or writing the file.
    Io(std::io::Error),
    /// The TOML content could not be deserialized.
    Deserialize(toml::de::Error),
    /// The TOML content could not be serialized.
    Serialize(toml::ser::Error),
    /// The history file version is not supported.
    UnsupportedVersion(u32),
}

#[cfg(feature = "toml")]
impl std::fmt::Display for HistoryFileError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(e) => write!(f, "{e}"),
            Self::Deserialize(e) => write!(f, "{e}"),
            Self::Serialize(e) => write!(f, "{e}"),
            Self::UnsupportedVersion(v) => write!(
                f,
                "unsupported history file version: {v}; expected {HISTORY_FILE_VERSION}"
            ),
        }
    }
}

#[cfg(feature = "toml")]
impl std::error::Error for HistoryFileError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(e) => Some(e),
            Self::Deserialize(e) => Some(e),
            Self::Serialize(e) => Some(e),
            Self::UnsupportedVersion(_) => None,
        }
    }
}

#[cfg(feature = "toml")]
impl From<std::io::Error> for HistoryFileError {
    fn from(e: std::io::Error) -> Self {
        Self::Io(e)
    }
}

#[cfg(feature = "toml")]
impl From<toml::de::Error> for HistoryFileError {
    fn from(e: toml::de::Error) -> Self {
        Self::Deserialize(e)
    }
}

#[cfg(feature = "toml")]
impl From<toml::ser::Error> for HistoryFileError {
    fn from(e: toml::ser::Error) -> Self {
        Self::Serialize(e)
    }
}
