use crate::prelude::*;

use toasty_core::schema::db::Migration;

// Each test calls `db.reset_db().await?` first so `__toasty_migrations` is
// clean.  All tests are `serial` to avoid races on that shared table.

/// A fresh database has no applied migrations.
#[driver_test(serial, requires(and(sql, migrations)))]
pub async fn migration_fresh_db_has_no_applied(t: &mut Test) -> Result<()> {
    let db = t.setup_db(models!()).await;
    db.reset_db().await?;

    let mut conn = db.driver().connect().await?;
    let applied = conn.applied_migrations().await?;

    assert!(
        applied.is_empty(),
        "expected no applied migrations, got {} entries",
        applied.len()
    );

    Ok(())
}

/// Applying a migration records its ID.
#[driver_test(serial, requires(and(sql, migrations)))]
pub async fn migration_apply_records_id(t: &mut Test) -> Result<()> {
    let db = t.setup_db(models!()).await;
    db.reset_db().await?;

    let mut conn = db.driver().connect().await?;

    let migration = Migration::new_sql(vec![
        "CREATE TABLE IF NOT EXISTS _test_mig_apply (id INTEGER PRIMARY KEY)".to_string(),
    ]);
    conn.apply_migration(101, "test_migration".to_string(), &migration)
        .await?;

    let applied = conn.applied_migrations().await?;
    assert_eq!(applied.len(), 1);
    assert_eq!(applied[0].id(), 101);

    Ok(())
}

/// Applying multiple migrations records all IDs in order.
#[driver_test(serial, requires(and(sql, migrations)))]
pub async fn migration_apply_multiple_in_order(t: &mut Test) -> Result<()> {
    let db = t.setup_db(models!()).await;
    db.reset_db().await?;

    let mut conn = db.driver().connect().await?;

    for (id, name) in [(1u64, "first"), (2, "second"), (3, "third")] {
        let migration = Migration::new_sql(vec![format!(
            "CREATE TABLE IF NOT EXISTS _test_multi_{name} (id INTEGER PRIMARY KEY)"
        )]);
        conn.apply_migration(id, name.to_string(), &migration)
            .await?;
    }

    let applied = conn.applied_migrations().await?;
    assert_eq!(applied.len(), 3);

    let ids: Vec<u64> = applied.iter().map(|m| m.id()).collect();
    assert!(ids.contains(&1));
    assert!(ids.contains(&2));
    assert!(ids.contains(&3));

    Ok(())
}

/// A migration that fails is not recorded as applied.
#[driver_test(serial, requires(and(sql, migrations)))]
pub async fn migration_apply_failure_not_recorded(t: &mut Test) -> Result<()> {
    let db = t.setup_db(models!()).await;
    db.reset_db().await?;

    let mut conn = db.driver().connect().await?;

    let migration = Migration::new_sql(vec!["THIS IS NOT VALID SQL AND SHOULD FAIL".to_string()]);
    let result = conn
        .apply_migration(999, "bad_migration".to_string(), &migration)
        .await;

    assert!(result.is_err(), "expected apply to fail on invalid SQL");

    let applied = conn.applied_migrations().await?;
    assert!(
        applied.iter().all(|m| m.id() != 999),
        "failed migration should not be recorded"
    );

    Ok(())
}

/// A two-statement migration where both succeed records the ID.
#[driver_test(serial, requires(and(sql, migrations)))]
pub async fn migration_apply_multi_statement_success(t: &mut Test) -> Result<()> {
    let db = t.setup_db(models!()).await;
    db.reset_db().await?;

    let mut conn = db.driver().connect().await?;

    let migration = Migration::new_sql(vec![
        "CREATE TABLE IF NOT EXISTS _test_multi_stmt (id INTEGER PRIMARY KEY, email TEXT NOT NULL)"
            .to_string(),
        "CREATE INDEX IF NOT EXISTS idx_multi_stmt_email ON _test_multi_stmt (email)".to_string(),
    ]);
    conn.apply_migration(42, "multi_stmt".to_string(), &migration)
        .await?;

    let applied = conn.applied_migrations().await?;
    assert_eq!(applied.len(), 1);
    assert_eq!(applied[0].id(), 42);

    Ok(())
}
