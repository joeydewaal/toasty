use super::Expr;
use crate::schema::{app::ModelId, db::TableId};

/// Identifies a row image produced by an update.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum MutationImage {
    /// The row before the update.
    Old,

    /// The row after the update.
    New,
}

/// An update row available to a returning expression.
///
/// Projecting a field from this expression references that field in either the
/// old or new row image.
#[derive(Clone, Debug, PartialEq)]
pub enum ExprMutation {
    /// An application-model row before lowering.
    Model {
        /// The model being updated.
        model: ModelId,
        /// The requested row image.
        image: MutationImage,
    },

    /// A database-table row after lowering.
    Table {
        /// The table being updated.
        table: TableId,
        /// The requested row image.
        image: MutationImage,
    },
}

impl ExprMutation {
    /// Creates an old application-model row.
    pub fn old_model(model: ModelId) -> Self {
        Self::Model {
            model,
            image: MutationImage::Old,
        }
    }

    /// Creates a new application-model row.
    pub fn new_model(model: ModelId) -> Self {
        Self::Model {
            model,
            image: MutationImage::New,
        }
    }

    /// Creates an old database-table row.
    pub fn old_table(table: TableId) -> Self {
        Self::Table {
            table,
            image: MutationImage::Old,
        }
    }

    /// Creates a new database-table row.
    pub fn new_table(table: TableId) -> Self {
        Self::Table {
            table,
            image: MutationImage::New,
        }
    }

    /// Returns the selected row image.
    pub fn image(&self) -> MutationImage {
        match self {
            Self::Model { image, .. } | Self::Table { image, .. } => *image,
        }
    }
}

impl From<ExprMutation> for Expr {
    fn from(value: ExprMutation) -> Self {
        Self::Mutation(value)
    }
}
