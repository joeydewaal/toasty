use super::{GenerateCommand, HistoryFile, apply_migrations};
use crate::{Config, MigrationConfig};
use std::path::Path;
use tempfile::TempDir;
use toasty::Db;

// ── Helpers ──────────────────────────────────────────────────────────────────

/// Creates a temp directory that holds both migration files and the SQLite DB.
/// Returns the dir (kept alive by the caller), a config pointing at it, and
/// the path to use for the SQLite database file.
fn temp_dir_and_config() -> (TempDir, Config) {
    let dir = tempfile::tempdir().unwrap();
    let config = Config {
        migration: MigrationConfig {
            path: dir.path().join("toasty"),
            ..MigrationConfig::default()
        },
    };
    (dir, config)
}

/// Builds a `Db` with a single `User` model backed by a file-based SQLite DB.
/// Using a file (not `:memory:`) means successive `connect()` calls share state.
async fn db_with_user(db_path: &Path) -> Db {
    #[derive(Debug, toasty::Model)]
    #[allow(dead_code)]
    struct User {
        #[key]
        id: String,
        name: String,
    }

    let url = format!("sqlite:{}", db_path.display());
    let mut builder = Db::builder();
    builder.register::<User>();
    builder.connect(&url).await.unwrap()
}

/// Builds a `Db` with `User` + `Todo` models backed by a file-based SQLite DB.
async fn db_with_user_and_todo(db_path: &Path) -> Db {
    #[derive(Debug, toasty::Model)]
    #[allow(dead_code)]
    struct User {
        #[key]
        id: String,
        name: String,
    }

    #[derive(Debug, toasty::Model)]
    #[allow(dead_code)]
    struct Todo {
        #[key]
        id: String,
        title: String,
    }

    let url = format!("sqlite:{}", db_path.display());
    let mut builder = Db::builder();
    builder.register::<User>();
    builder.register::<Todo>();
    builder.connect(&url).await.unwrap()
}

// ── Generate tests ────────────────────────────────────────────────────────────

#[tokio::test]
async fn generate_creates_initial_migration() {
    let (dir, config) = temp_dir_and_config();
    let db = db_with_user(&dir.path().join("db.sqlite")).await;

    GenerateCommand { name: None }.run(&db, &config).unwrap();

    let history = HistoryFile::load(config.migration.get_history_file_path()).unwrap();
    assert_eq!(history.migrations().len(), 1);
    assert_eq!(history.migrations()[0].snapshot_name, "0000_snapshot.toml");
    assert!(
        config
            .migration
            .get_snapshots_dir()
            .join("0000_snapshot.toml")
            .exists()
    );
}

#[tokio::test]
async fn generate_no_diff_is_noop() {
    let (dir, config) = temp_dir_and_config();
    let db = db_with_user(&dir.path().join("db.sqlite")).await;

    GenerateCommand { name: None }.run(&db, &config).unwrap();
    GenerateCommand { name: None }.run(&db, &config).unwrap();

    let history = HistoryFile::load(config.migration.get_history_file_path()).unwrap();
    assert_eq!(history.migrations().len(), 1);
    assert!(
        !config
            .migration
            .get_snapshots_dir()
            .join("0001_snapshot.toml")
            .exists()
    );
}

#[tokio::test]
async fn generate_increments_migration_number() {
    let (dir, config) = temp_dir_and_config();
    let db_path = dir.path().join("db.sqlite");

    // First generate: User only
    let db1 = db_with_user(&db_path).await;
    GenerateCommand { name: None }.run(&db1, &config).unwrap();

    // Second generate: User + Todo (new table → non-interactive diff)
    let db2 = db_with_user_and_todo(&db_path).await;
    GenerateCommand { name: None }.run(&db2, &config).unwrap();

    let history = HistoryFile::load(config.migration.get_history_file_path()).unwrap();
    assert_eq!(history.migrations().len(), 2);
    assert_eq!(history.migrations()[1].snapshot_name, "0001_snapshot.toml");
    assert!(
        config
            .migration
            .get_snapshots_dir()
            .join("0001_snapshot.toml")
            .exists()
    );
}

#[tokio::test]
async fn generate_custom_name() {
    let (dir, config) = temp_dir_and_config();
    let db = db_with_user(&dir.path().join("db.sqlite")).await;

    GenerateCommand {
        name: Some("init".to_string()),
    }
    .run(&db, &config)
    .unwrap();

    let history = HistoryFile::load(config.migration.get_history_file_path()).unwrap();
    assert_eq!(history.migrations()[0].name, "init");
}

// ── Apply tests ──────────────────────────────────────────────────────────────

#[tokio::test]
async fn apply_empty_history() {
    let (dir, config) = temp_dir_and_config();
    let db = db_with_user(&dir.path().join("db.sqlite")).await;

    // No history.toml — should return Ok without error
    apply_migrations(&db, &config).await.unwrap();
}

#[tokio::test]
async fn apply_runs_pending_migration() {
    let (dir, config) = temp_dir_and_config();
    let db_path = dir.path().join("db.sqlite");
    let db = db_with_user(&db_path).await;

    GenerateCommand { name: None }.run(&db, &config).unwrap();
    apply_migrations(&db, &config).await.unwrap();

    let expected_id = {
        let history = HistoryFile::load(config.migration.get_history_file_path()).unwrap();
        history.migrations()[0].id
    };

    let mut conn = db.driver().connect().await.unwrap();
    let applied = conn.applied_migrations().await.unwrap();
    assert_eq!(applied.len(), 1);
    assert_eq!(applied[0].id(), expected_id);
}

#[tokio::test]
async fn apply_is_idempotent() {
    let (dir, config) = temp_dir_and_config();
    let db_path = dir.path().join("db.sqlite");
    let db = db_with_user(&db_path).await;

    GenerateCommand { name: None }.run(&db, &config).unwrap();
    apply_migrations(&db, &config).await.unwrap();
    apply_migrations(&db, &config).await.unwrap(); // second apply is a no-op

    let mut conn = db.driver().connect().await.unwrap();
    let applied = conn.applied_migrations().await.unwrap();
    assert_eq!(applied.len(), 1);
}

#[tokio::test]
async fn apply_two_migrations() {
    let (dir, config) = temp_dir_and_config();
    let db_path = dir.path().join("db.sqlite");

    // Generate migration 1: User only
    let db1 = db_with_user(&db_path).await;
    GenerateCommand { name: None }.run(&db1, &config).unwrap();

    // Generate migration 2: User + Todo
    let db2 = db_with_user_and_todo(&db_path).await;
    GenerateCommand { name: None }.run(&db2, &config).unwrap();

    // Apply both
    apply_migrations(&db2, &config).await.unwrap();

    let mut conn = db2.driver().connect().await.unwrap();
    let applied = conn.applied_migrations().await.unwrap();
    assert_eq!(applied.len(), 2);
}
