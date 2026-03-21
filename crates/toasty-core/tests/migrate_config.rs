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

#[test]
fn config_load_or_default_missing_file() {
    let cfg = Config::load_or_default("/nonexistent/path/Toasty.toml").unwrap();
    assert_eq!(cfg.migration.path, PathBuf::from("toasty"));
}

#[test]
fn config_load_or_default_empty_file() {
    let path = std::env::temp_dir().join("toasty_test_config_empty.toml");
    std::fs::write(&path, "").unwrap();
    let cfg = Config::load_or_default(&path).unwrap();
    assert_eq!(cfg.migration.path, PathBuf::from("toasty"));
    std::fs::remove_file(&path).ok();
}

#[test]
fn config_load_or_default_custom_path() {
    let path = std::env::temp_dir().join("toasty_test_config_custom.toml");
    std::fs::write(&path, "[migration]\npath = \"db/migrations\"\n").unwrap();
    let cfg = Config::load_or_default(&path).unwrap();
    assert_eq!(cfg.migration.path, PathBuf::from("db/migrations"));
    std::fs::remove_file(&path).ok();
}

#[test]
fn config_load_or_default_invalid_toml() {
    let path = std::env::temp_dir().join("toasty_test_config_invalid.toml");
    std::fs::write(&path, "[[not valid toml{{").unwrap();
    let err = Config::load_or_default(&path).unwrap_err();
    assert!(err.is_migration_failed());
    std::fs::remove_file(&path).ok();
}
