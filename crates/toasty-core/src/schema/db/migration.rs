/// A database migration generated from a [`SchemaDiff`](super::SchemaDiff) by a driver.
///
/// Currently only SQL migrations are supported.
///
/// # Examples
///
/// ```ignore
/// use toasty_core::schema::db::Migration;
///
/// let m = Migration::new_sql(vec!["CREATE TABLE users (id INTEGER PRIMARY KEY)".to_string()]);
/// assert_eq!(m.statements(), &["CREATE TABLE users (id INTEGER PRIMARY KEY)"]);
/// ```
pub enum Migration {
    /// A SQL migration containing one or more statements.
    Sql(Vec<String>),
}

impl Migration {
    /// Creates a SQL migration from a list of SQL statements.
    pub fn new_sql(statements: Vec<String>) -> Self {
        Migration::Sql(statements)
    }

    /// Returns the individual SQL statements in this migration.
    pub fn statements(&self) -> &[String] {
        match self {
            Migration::Sql(stmts) => stmts,
        }
    }
}

/// Metadata about a migration that has already been applied to a database.
///
/// Stores the unique migration ID assigned by the migration system.
///
/// # Examples
///
/// ```ignore
/// use toasty_core::schema::db::AppliedMigration;
///
/// let applied = AppliedMigration::new(42);
/// assert_eq!(applied.id(), 42);
/// ```
pub struct AppliedMigration {
    id: u64,
}

impl AppliedMigration {
    /// Creates a new `AppliedMigration` with the given ID.
    pub fn new(id: u64) -> Self {
        Self { id }
    }

    /// Returns the migration's unique ID.
    pub fn id(&self) -> u64 {
        self.id
    }
}
