#![cfg(feature = "sqlite")]

use toasty::migrate;

async fn setup_db() -> toasty::Db {
    let driver = toasty_driver_sqlite::Sqlite::in_memory();
    toasty::Db::builder()
        .models(toasty::models!())
        .build(driver)
        .await
        .unwrap()
}

/// Verify a table exists by inserting a row through the pool connection.
async fn assert_table_exists(conn: &toasty::db::Connection, sql: &str) -> toasty::Result<()> {
    conn.apply_migrations(vec![(u64::MAX, "probe.sql".to_owned(), sql.to_owned())])
        .await?;
    Ok(())
}

#[tokio::test]
async fn migrate_apply_pending() -> toasty::Result<()> {
    let db = setup_db().await;
    let conn = db.connection().await?;

    let count = conn
        .apply_migrations(vec![(
            8000000000000001,
            "0000_test.sql".to_owned(),
            "CREATE TABLE \"apply_pending_test\" (\"id\" INTEGER NOT NULL, PRIMARY KEY (\"id\"))"
                .to_owned(),
        )])
        .await?;
    assert_eq!(count, 1);

    // Verify the table exists by inserting a row
    assert_table_exists(
        &conn,
        "INSERT INTO \"apply_pending_test\" (\"id\") VALUES (1)",
    )
    .await?;

    Ok(())
}

#[tokio::test]
async fn migrate_apply_pending_idempotent() -> toasty::Result<()> {
    let db = setup_db().await;

    let migrations = vec![(
        8000000000000002,
        "0000_test.sql".to_owned(),
        "CREATE TABLE \"apply_pending_idemp\" (\"id\" INTEGER NOT NULL, PRIMARY KEY (\"id\"))"
            .to_owned(),
    )];

    let conn = db.connection().await?;
    let first = conn.apply_migrations(migrations.clone()).await?;
    assert_eq!(first, 1);

    let second = conn.apply_migrations(migrations).await?;
    assert_eq!(second, 0);

    // Verify the table exists
    assert_table_exists(
        &conn,
        "INSERT INTO \"apply_pending_idemp\" (\"id\") VALUES (1)",
    )
    .await?;

    Ok(())
}

#[tokio::test]
async fn migrate_apply_pending_skips_applied() -> toasty::Result<()> {
    let db = setup_db().await;
    let conn = db.connection().await?;

    // Apply only the first migration
    let count = conn
        .apply_migrations(vec![(
            8000000000000003,
            "0000_first.sql".to_owned(),
            "CREATE TABLE \"skip_test\" (\"id\" INTEGER NOT NULL, PRIMARY KEY (\"id\"))".to_owned(),
        )])
        .await?;
    assert_eq!(count, 1);

    // Now apply both — only the second should run
    let count = conn
        .apply_migrations(vec![
            (
                8000000000000003,
                "0000_first.sql".to_owned(),
                "CREATE TABLE \"skip_test\" (\"id\" INTEGER NOT NULL, PRIMARY KEY (\"id\"))"
                    .to_owned(),
            ),
            (
                8000000000000004,
                "0001_second.sql".to_owned(),
                "ALTER TABLE \"skip_test\" ADD COLUMN \"name\" TEXT".to_owned(),
            ),
        ])
        .await?;
    assert_eq!(count, 1);

    // Verify both the table and added column exist
    assert_table_exists(
        &conn,
        "INSERT INTO \"skip_test\" (\"id\", \"name\") VALUES (1, 'hello')",
    )
    .await?;

    Ok(())
}

#[tokio::test]
async fn migrate_macro_embeds_and_applies() -> toasty::Result<()> {
    let db = setup_db().await;

    let migrator = migrate!("fixtures");
    let count = migrator.exec(&db).await?;
    assert_eq!(count, 2);

    // Running again should be a no-op
    let count = migrator.exec(&db).await?;
    assert_eq!(count, 0);

    // Verify the table and columns exist (table created by first migration,
    // "score" column added by second)
    let conn = db.connection().await?;
    assert_table_exists(
        &conn,
        "INSERT INTO \"migrate_test_items\" (\"id\", \"name\", \"score\") VALUES (1, 'test', 42)",
    )
    .await?;

    Ok(())
}
