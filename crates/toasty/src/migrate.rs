mod migrator;

pub use migrator::{EmbeddedMigration, Migrator};
pub use toasty_core::migrate::{
    Config, HistoryFile, HistoryFileMigration, MigrationConfig, MigrationPrefixStyle,
};
