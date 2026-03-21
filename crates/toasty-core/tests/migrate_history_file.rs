#![cfg(feature = "migrate")]

use toasty_core::migrate::{HistoryFile, HistoryFileMigration};

fn migration(id: u64, name: &str) -> HistoryFileMigration {
    HistoryFileMigration {
        id,
        name: name.to_string(),
        snapshot_name: format!("{name}.snapshot"),
        checksum: None,
    }
}

#[test]
fn new_is_empty() {
    let hf = HistoryFile::new();
    assert!(hf.migrations().is_empty());
}

#[test]
fn load_or_default_missing_file() {
    let hf = HistoryFile::load_or_default("/nonexistent/history.toml").unwrap();
    assert!(hf.migrations().is_empty());
}

#[test]
fn parse_valid() {
    let toml = r#"
version = 1

[[migrations]]
id = 1
name = "0001_init.sql"
snapshot_name = "0001_init.snapshot"
"#;
    let hf: HistoryFile = toml.parse().unwrap();
    assert_eq!(hf.migrations().len(), 1);
    assert_eq!(hf.migrations()[0].id, 1);
    assert_eq!(hf.migrations()[0].name, "0001_init.sql");
}

#[test]
fn parse_wrong_version() {
    let toml = "version = 99\n";
    let err = toml.parse::<HistoryFile>().unwrap_err();
    assert!(err.is_migration_failed());
    assert!(err.to_string().contains("99"));
}

#[test]
fn next_migration_number_empty() {
    assert_eq!(HistoryFile::new().next_migration_number(), 0);
}

#[test]
fn next_migration_number_increments() {
    let mut hf = HistoryFile::new();
    hf.add_migration(migration(1, "0003_add_users.sql"));
    assert_eq!(hf.next_migration_number(), 4);
}

#[test]
fn add_and_remove_migration() {
    let mut hf = HistoryFile::new();
    hf.add_migration(migration(1, "0001_init.sql"));
    hf.add_migration(migration(2, "0002_users.sql"));
    assert_eq!(hf.migrations().len(), 2);

    hf.remove_migration(0);
    assert_eq!(hf.migrations().len(), 1);
    assert_eq!(hf.migrations()[0].name, "0002_users.sql");
}

#[test]
fn display_roundtrip() {
    let mut hf = HistoryFile::new();
    hf.add_migration(migration(42, "0001_init.sql"));

    let serialized = hf.to_string();
    let parsed: HistoryFile = serialized.parse().unwrap();
    assert_eq!(parsed.migrations().len(), 1);
    assert_eq!(parsed.migrations()[0].id, 42);
    assert_eq!(parsed.migrations()[0].name, "0001_init.sql");
}

#[test]
fn save_and_load() {
    let path = std::env::temp_dir().join("toasty_test_history_file.toml");
    let mut hf = HistoryFile::new();
    hf.add_migration(migration(7, "0001_init.sql"));
    hf.save(&path).unwrap();

    let loaded = HistoryFile::load(&path).unwrap();
    assert_eq!(loaded.migrations().len(), 1);
    assert_eq!(loaded.migrations()[0].id, 7);
    std::fs::remove_file(&path).ok();
}
