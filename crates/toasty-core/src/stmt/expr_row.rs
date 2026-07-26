use super::Expr;
use crate::schema::{app::ModelId, db::TableId};

/// Identifies which row image a [`ExprRow`] refers to.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum RowImage {
    /// The row proposed by an upsert's create branch, rather than the row
    /// already stored in the conflicting row. Serializes to the backend's
    /// proposed-row relation, such as PostgreSQL's `EXCLUDED`.
    Incoming,

    /// The row before an update.
    Old,

    /// The row after an update.
    New,
}

/// A qualified row made available to an expression by the surrounding statement.
///
/// Projecting a field from this expression references that field in the named
/// row image rather than in the statement's own source row. SQL serializers map
/// the projected expression to the backend's syntax for that image — `excluded.`
/// for an upsert's proposed row, `old.` for an update's pre-update row, and an
/// unqualified column for an update's post-update row.
#[derive(Clone, Debug, PartialEq)]
pub enum ExprRow {
    /// An application-model row before lowering.
    Model {
        /// The model the row belongs to.
        model: ModelId,
        /// The requested row image.
        image: RowImage,
    },

    /// A database-table row after lowering.
    Table {
        /// The table the row belongs to.
        table: TableId,
        /// The requested row image.
        image: RowImage,
    },
}

impl ExprRow {
    /// Creates an application-model row for the given image.
    pub fn model(model: ModelId, image: RowImage) -> Self {
        Self::Model { model, image }
    }

    /// Creates a database-table row for the given image.
    pub fn table(table: TableId, image: RowImage) -> Self {
        Self::Table { table, image }
    }

    /// Returns the selected row image.
    pub fn image(&self) -> RowImage {
        match self {
            Self::Model { image, .. } | Self::Table { image, .. } => *image,
        }
    }
}

impl From<ExprRow> for Expr {
    fn from(value: ExprRow) -> Self {
        Self::Row(value)
    }
}
