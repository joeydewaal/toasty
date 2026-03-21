use super::Error;

/// Error when a migration operation fails.
///
/// This occurs when:
/// - A migration configuration file cannot be read or parsed
/// - The migration history file cannot be read or written
/// - The history file version is unsupported
#[derive(Debug)]
pub(super) struct MigrationFailed {
    message: Box<str>,
}

impl std::error::Error for MigrationFailed {}

impl core::fmt::Display for MigrationFailed {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        write!(f, "migration failed: {}", self.message)
    }
}

impl Error {
    /// Creates a migration failed error.
    pub fn migration_failed(message: impl Into<String>) -> Error {
        Error::from(super::ErrorKind::MigrationFailed(MigrationFailed {
            message: message.into().into(),
        }))
    }

    /// Returns `true` if this error is a migration failed error.
    pub fn is_migration_failed(&self) -> bool {
        matches!(self.kind(), super::ErrorKind::MigrationFailed(_))
    }
}
