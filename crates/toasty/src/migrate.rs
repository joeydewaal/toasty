use crate::Result;
use std::collections::HashSet;
use toasty_core::schema::db::Migration;

/// A single migration embedded into the binary at compile time via [`migrate!`].
pub struct EmbeddedMigration {
    /// Unique identifier sourced from `history.toml`.
    pub id: u64,
    /// File name of this migration (e.g. `"0001_migration.sql"`).
    pub name: &'static str,
    /// SQL content embedded at compile time.
    pub sql: &'static str,
}

/// A set of migrations embedded at compile time, ready to apply to a database.
///
/// Produced by the [`migrate!`] macro. Call [`exec`](Migrator::exec) to apply
/// all pending migrations.
pub struct Migrator {
    migrations: &'static [EmbeddedMigration],
}

impl Migrator {
    #[doc(hidden)]
    pub const fn new(migrations: &'static [EmbeddedMigration]) -> Self {
        Self { migrations }
    }

    /// Apply all pending migrations to the database.
    ///
    /// Migrations that have already been applied (tracked in the
    /// `__toasty_migrations` table) are skipped.
    pub async fn exec(self, db: &mut crate::Db) -> Result<()> {
        let mut conn = db.driver().connect().await?;

        let applied = conn.applied_migrations().await?;
        let applied_ids: HashSet<u64> = applied.iter().map(|m| m.id()).collect();

        for m in self.migrations {
            if applied_ids.contains(&m.id) {
                continue;
            }
            let migration = Migration::new_sql(m.sql.to_string());
            conn.apply_migration(m.id, m.name.to_string(), &migration)
                .await?;
        }

        Ok(())
    }
}
