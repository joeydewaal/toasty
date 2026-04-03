//! Embedded migration support.
//!
//! The [`migrate!`](crate::migrate) macro reads migration files at compile time
//! and produces a [`Migrator`] that can apply them at runtime. The
//! [`apply_pending`] helper contains the shared "diff and apply" logic used by
//! both the macro-based [`Migrator`] and the CLI.

use crate::Db;

use std::collections::HashSet;
use toasty_core::driver::Connection;
use toasty_core::schema::db::Migration;

/// A single migration embedded at compile time by [`migrate!`](crate::migrate).
pub struct EmbeddedMigration {
    /// Unique identifier assigned when the migration was generated.
    pub id: u64,
    /// File name of the migration (e.g. `"0000_migration.sql"`).
    pub name: &'static str,
    /// Raw SQL content of the migration file.
    pub sql: &'static str,
}

/// A set of compile-time-embedded migrations.
///
/// Created by the [`migrate!`](crate::migrate) macro. Call [`exec`](Migrator::exec)
/// to apply any pending migrations to the database.
///
/// # Examples
///
/// ```ignore
/// // Embed migrations from the default `toasty/` directory:
/// toasty::migrate!().exec(&db).await?;
///
/// // Embed from a custom path relative to Cargo.toml:
/// toasty::migrate!("../migrations").exec(&db).await?;
/// ```
pub struct Migrator {
    migrations: &'static [EmbeddedMigration],
}

impl Migrator {
    /// Creates a new `Migrator` from a static slice of embedded migrations.
    ///
    /// This is called by the generated code from [`migrate!`](crate::migrate)
    /// and is not intended to be used directly.
    pub const fn new(migrations: &'static [EmbeddedMigration]) -> Self {
        Self { migrations }
    }

    /// Applies all pending migrations to the database.
    ///
    /// Migrations that have already been applied (tracked by ID) are skipped.
    /// Returns the number of migrations that were applied.
    pub async fn exec(&self, db: &Db) -> crate::Result<usize> {
        let conn = db.connection().await?;
        let tuples: Vec<(u64, String, String)> = self
            .migrations
            .iter()
            .map(|m| (m.id, m.name.to_owned(), m.sql.to_owned()))
            .collect();
        conn.apply_migrations(tuples).await
    }
}

/// Applies pending migrations to a database connection.
///
/// Given a list of `(id, name, sql)` migration tuples, queries the connection
/// for already-applied migration IDs and applies only those not yet present.
/// Returns the number of newly applied migrations.
///
/// This is the shared core used by both [`Migrator::exec`] and the Toasty CLI.
pub(crate) async fn apply_pending(
    conn: &mut dyn Connection,
    migrations: &[(u64, &str, &str)],
) -> crate::Result<usize> {
    let applied = conn.applied_migrations().await?;
    let applied_ids: HashSet<u64> = applied.iter().map(|m| m.id()).collect();

    let mut count = 0;
    for &(id, name, sql) in migrations {
        if applied_ids.contains(&id) {
            continue;
        }
        let migration = Migration::new_sql(sql.to_owned());
        conn.apply_migration(id, name.to_owned(), &migration)
            .await?;
        count += 1;
    }

    Ok(count)
}
