use super::Expr;
use crate::schema::{app::ModelId, db::TableId};

/// Identifies the model or table belonging to an [`ExprRow`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ExprRowTarget {
    /// An application model before lowering.
    Model(ModelId),

    /// A database table after lowering.
    Table(TableId),
}

/// A qualified row made available to an expression by the surrounding statement.
///
/// Projecting a field from this expression references that field in the named
/// row rather than in the statement's source row.
#[derive(Clone, Debug, PartialEq)]
pub enum ExprRow {
    /// The row proposed by an upsert's create branch.
    Incoming(ExprRowTarget),

    /// The row before an update.
    Old(ExprRowTarget),
}

impl ExprRow {
    /// Creates an application-model row proposed by an upsert.
    pub fn incoming_model(model: ModelId) -> Self {
        Self::Incoming(ExprRowTarget::Model(model))
    }

    /// Creates a database-table row proposed by an upsert.
    pub fn incoming_table(table: TableId) -> Self {
        Self::Incoming(ExprRowTarget::Table(table))
    }

    /// Creates an application-model row as it was before an update.
    pub fn old_model(model: ModelId) -> Self {
        Self::Old(ExprRowTarget::Model(model))
    }

    /// Creates a database-table row as it was before an update.
    pub fn old_table(table: TableId) -> Self {
        Self::Old(ExprRowTarget::Table(table))
    }

    /// Returns the model or table belonging to this row.
    pub fn target(&self) -> ExprRowTarget {
        match self {
            Self::Incoming(target) | Self::Old(target) => *target,
        }
    }
}

impl From<ExprRow> for Expr {
    fn from(value: ExprRow) -> Self {
        Self::Row(value)
    }
}
