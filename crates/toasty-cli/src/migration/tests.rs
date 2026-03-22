use super::{apply_migrations, GenerateCommand, HistoryFile};
use crate::{Config, MigrationConfig};
use std::path::PathBuf;
use tempfile::TempDir;
use toasty::Db;

// ── Helpers ──────────────────────────────────────────────────────────────────

fn temp_config() -> (TempDir, Config) {
    let dir = tempfile::tempdir().unwrap();
    let config = Config {
        migration: MigrationConfig {
            path: dir.path().to_path_buf(),
            ..MigrationConfig::default()
        },
    };
    (dir, config)
}

async fn db_with_user() -> Db {
    #[derive(Debug, toasty::Model)]
    #[allow(dead_code)]
    struct User {
        #[key]
        #[auto]
        id: i64,
        name: String,
    }

    let mut builder = Db::builder();
    builder.register::<User>();
    builder.connect("sqlite::memory:").await.unwrap()
}

async fn db_with_user_and_todo() -> Db {
    #[derive(Debug, toasty::Model)]
    #[allow(dead_code)]
    struct User {
        #[key]
        #[auto]
        id: i64,
        name: String,
    }

    #[derive(Debug, toasty::Model)]
    #[allow(dead_code)]
    struct Todo {
        #[key]
        #[auto]
        id: i64,
        title: String,
    }

    let mut builder = Db::builder();
    builder.register::<User>();
    builder.register::<Todo>();
    builder.connect("sqlite::memory:").await.unwrap()
}

fn snapshot_path(config: &Config, name: &str) -> PathBuf {
    config.migration.get_snapshots_dir().join(name)
}

// ── Generate tests ───────────────────────────────────────────────────────────

#[tokio::test]
async fn generate_creates_initial_migration() {
    let (_dir, config) = temp_config();
    let db = db_with_user().await;

    GenerateCommand { name: None }.run(&db, &config).unwrap();

    let history = HistoryFile::load(config.migration.get_history_file_path()).unwrap();
    assert_eq!(history.migrations().len(), 1);
    assert_eq!(history.migrations()[0].snapshot_name, "0000_snapshot.toml");
    assert!(snapshot_path(&config, "0000_snapshot.toml").exists());
}

#[tokio::test]
async fn generate_no_diff_is_noop() {
    let (_dir, config) = temp_config();
    let db = db_with_user().await;

    GenerateCommand { name: None }.run(&db, &config).unwrap();
    GenerateCommand { name: None }.run(&db, &config).unwrap();

    let history = HistoryFile::load(config.migration.get_history_file_path()).unwrap();
    assert_eq!(history.migrations().len(), 1);
    assert!(!snapshot_path(&config, "0001_snapshot.toml").exists());
}

#[tokio::test]
async fn generate_increments_migration_number() {
    let (_dir, config) = temp_config();

    // First generate: User only
    let db1 = db_with_user().await;
    GenerateCommand { name: None }.run(&db1, &config).unwrap();

    // Second generate: User + Todo (new table added, no interactive prompts needed)
    let db2 = db_with_user_and_todo().await;
    GenerateCommand { name: None }.run(&db2, &config).unwrap();

    let history = HistoryFile::load(config.migration.get_history_file_path()).unwrap();
    assert_eq!(history.migrations().len(), 2);
    assert_eq!(history.migrations()[1].snapshot_name, "0001_snapshot.toml");
    assert!(snapshot_path(&config, "0001_snapshot.toml").exists());
}

#[tokio::test]
async fn generate_custom_name() {
    let (_dir, config) = temp_config();
    let db = db_with_user().await;

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
    let (_dir, config) = temp_config();
    let db = db_with_user().await;

    // No history.toml — should return Ok without error
    apply_migrations(&db, &config).await.unwrap();
}

#[tokio::test]
async fn apply_runs_pending_migration() {
    let (_dir, config) = temp_config();
    let db = db_with_user().await;

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
    let (_dir, config) = temp_config();
    let db = db_with_user().await;

    GenerateCommand { name: None }.run(&db, &config).unwrap();
    apply_migrations(&db, &config).await.unwrap();
    apply_migrations(&db, &config).await.unwrap(); // second apply is a no-op

    let mut conn = db.driver().connect().await.unwrap();
    let applied = conn.applied_migrations().await.unwrap();
    assert_eq!(applied.len(), 1);
}

#[tokio::test]
async fn apply_two_migrations() {
    let (_dir, config) = temp_config();

    // Generate migration 1: User
    let db1 = db_with_user().await;
    GenerateCommand { name: None }.run(&db1, &config).unwrap();

    // Generate migration 2: User + Todo
    let db2 = db_with_user_and_todo().await;
    GenerateCommand { name: None }.run(&db2, &config).unwrap();

    // Apply both using db2 (which has the full schema)
    apply_migrations(&db2, &config).await.unwrap();

    let mut conn = db2.driver().connect().await.unwrap();
    let applied = conn.applied_migrations().await.unwrap();
    assert_eq!(applied.len(), 2);
}
