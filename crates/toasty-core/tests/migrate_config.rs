#![cfg(feature = "migrate")]

use std::path::PathBuf;
use toasty_core::migrate::{Config, MigrationConfig, MigrationPrefixStyle};

#[test]
fn migration_config_defaults() {
    let cfg = MigrationConfig::default();
    assert_eq!(cfg.path, PathBuf::from("toasty"));
    assert_eq!(cfg.prefix_style, MigrationPrefixStyle::Sequential);
    assert!(!cfg.checksums);
    assert!(cfg.statement_breakpoints);
}

#[test]
fn migration_config_derived_paths() {
    let cfg = MigrationConfig::default();
    assert_eq!(cfg.get_migrations_dir(), PathBuf::from("toasty/migrations"));
    assert_eq!(cfg.get_snapshots_dir(), PathBuf::from("toasty/snapshots"));
    assert_eq!(
        cfg.get_history_file_path(),
        PathBuf::from("toasty/history.toml")
    );
}
