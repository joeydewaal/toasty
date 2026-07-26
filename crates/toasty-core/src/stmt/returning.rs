use super::{Expr, Include};
use crate::stmt::{self, ExprSet, Node, Query, RowImage, Statement, Visit};

/// Specifies what data a statement returns.
///
/// Used both as the projection in `SELECT` queries and as the `RETURNING`
/// clause in `INSERT`, `UPDATE`, and `DELETE` statements.
///
/// # Examples
///
/// ```ignore
/// use toasty_core::stmt::Returning;
///
/// let ret = Returning::Model {
///     include: vec![],
///     old: false,
/// };
/// assert!(ret.is_model());
/// ```
#[derive(Debug, Clone, PartialEq)]
pub enum Returning {
    /// Return the full model with the specified association includes.
    Model {
        /// Associations that should be eagerly loaded, with optional
        /// per-relation filters.
        include: Vec<Include>,
        /// Return the model as it was before an update.
        old: bool,
    },

    /// Return whether the operation changed any rows.
    Changed,

    /// Return the number of rows affected by a mutation.
    Count,

    /// Return the result of evaluating an expression against the source rows.
    Project(Expr),

    /// Return a fixed expression, independent of the statement source.
    Expr(Expr),
}

impl Returning {
    /// Creates a `Returning::Project` from an iterator of expressions, combining
    /// them into a record expression.
    pub fn from_project_iter<T>(items: impl IntoIterator<Item = T>) -> Self
    where
        T: Into<Expr>,
    {
        Returning::Project(Expr::record(items))
    }

    /// Returns `true` if this is the `Model` variant.
    pub fn is_model(&self) -> bool {
        matches!(self, Self::Model { .. })
    }

    /// Returns the association includes for a `Model` variant, or an
    /// empty slice for other variants.
    pub fn model_includes(&self) -> &[Include] {
        match self {
            Self::Model { include, .. } => include,
            _ => &[],
        }
    }

    /// Returns a mutable reference to the `Model` variant's includes.
    ///
    /// # Panics
    ///
    /// Panics if this is not the `Model` variant.
    #[track_caller]
    pub fn model_includes_mut_unwrap(&mut self) -> &mut Vec<Include> {
        match self {
            Self::Model { include, .. } => include,
            _ => panic!("not a Model variant"),
        }
    }

    /// Returns `true` if this is the `Changed` variant.
    pub fn is_changed(&self) -> bool {
        matches!(self, Self::Changed)
    }

    /// Returns `true` if this is the `Count` variant.
    pub fn is_count(&self) -> bool {
        matches!(self, Self::Count)
    }

    /// Returns `true` if this is the `Project` variant.
    pub fn is_project(&self) -> bool {
        matches!(self, Self::Project(_))
    }

    /// Returns a reference to the inner expression if this is the `Project`
    /// variant.
    pub fn as_project(&self) -> Option<&Expr> {
        match self {
            Self::Project(expr) => Some(expr),
            _ => None,
        }
    }

    /// Returns `true` when this clause reads pre-mutation values.
    pub fn uses_old(&self) -> bool {
        self.uses_image(RowImage::Old)
    }

    /// Returns `true` when this clause reads post-mutation values.
    pub fn uses_new(&self) -> bool {
        self.uses_image(RowImage::New)
    }

    fn uses_image(&self, image: RowImage) -> bool {
        match self {
            Self::Model { old, .. } => {
                return match image {
                    RowImage::Old => *old,
                    RowImage::New => !*old,
                    // A model returning clause never reads an upsert's
                    // proposed row.
                    RowImage::Incoming => false,
                };
            }
            Self::Changed | Self::Count | Self::Expr(_) => return false,
            Self::Project(..) => {}
        }

        struct FindImage {
            image: RowImage,
            found: bool,
        }

        impl Visit for FindImage {
            fn visit_expr_row(&mut self, expr: &stmt::ExprRow) {
                self.found |= expr.image() == self.image;
            }
        }

        let expr = self.as_project().unwrap();
        let mut find = FindImage {
            image,
            found: false,
        };
        find.visit_expr(expr);
        find.found
    }

    /// Returns a reference to the inner expression.
    ///
    /// # Panics
    ///
    /// Panics if this is not the `Project` variant.
    #[track_caller]
    pub fn as_project_unwrap(&self) -> &Expr {
        self.as_project()
            .unwrap_or_else(|| panic!("expected stmt::Returning::Project; actual={self:#?}"))
    }

    /// Returns a mutable reference to the inner expression if this is the
    /// `Project` variant.
    pub fn as_project_mut(&mut self) -> Option<&mut Expr> {
        match self {
            Self::Project(expr) => Some(expr),
            _ => None,
        }
    }

    /// Returns a mutable reference to the inner expression.
    ///
    /// # Panics
    ///
    /// Panics if this is not the `Project` variant.
    #[track_caller]
    pub fn as_project_mut_unwrap(&mut self) -> &mut Expr {
        if !self.is_project() {
            panic!("expected stmt::Returning::Project; actual={self:#?}");
        }
        self.as_project_mut().unwrap()
    }

    /// Replaces this returning clause with `Returning::Project` containing the
    /// given expression.
    pub fn set_project(&mut self, expr: impl Into<Expr>) {
        *self = Returning::Project(expr.into());
    }

    /// Returns `true` if this is the `Expr` variant.
    pub fn is_expr(&self) -> bool {
        matches!(self, Self::Expr(..))
    }

    /// Takes this returning clause, replaces it with a null projection, and
    /// returns the original value.
    pub fn take(&mut self) -> Returning {
        std::mem::replace(self, Returning::Project(stmt::Expr::null()))
    }
}

impl Statement {
    /// Returns a reference to this statement's `RETURNING` clause, if present.
    ///
    /// Returns `None` if the statement does not have a `RETURNING` clause or is
    /// a statement type that does not support `RETURNING`.
    pub fn returning(&self) -> Option<&Returning> {
        match self {
            Statement::Delete(delete) => delete.returning.as_ref(),
            Statement::Insert(insert) => insert.returning.as_ref(),
            Statement::Query(query) => query.returning(),
            Statement::Update(update) => update.returning.as_ref(),
        }
    }

    /// Take the `Returning` clause
    pub fn take_returning(&mut self) -> Option<Returning> {
        match self {
            Statement::Delete(delete) => delete.returning.take(),
            Statement::Insert(insert) => insert.returning.take(),
            Statement::Query(query) => match &mut query.body {
                ExprSet::Select(select) => Some(select.returning.take()),
                ExprSet::Values(..) => None,
                _ => todo!("stmt={self:#?}"),
            },
            Statement::Update(update) => update.returning.take(),
        }
    }

    /// Set the `Returning` clause
    pub fn set_returning(&mut self, returning: Returning) {
        match self {
            Statement::Delete(delete) => delete.returning = Some(returning),
            Statement::Insert(insert) => insert.returning = Some(returning),
            Statement::Query(query) => *query.returning_mut_unwrap() = returning,
            Statement::Update(update) => update.returning = Some(returning),
        }
    }

    /// Set the `Returning` clause to `Returning::Project` containing the given
    /// expression.
    pub fn set_returning_project(&mut self, expr: impl Into<Expr>) {
        self.set_returning(Returning::Project(expr.into()));
    }

    /// Set the `Returning` clause to `Returning::Expr` containing the given
    /// expression.
    pub fn set_returning_expr(&mut self, expr: impl Into<Expr>) {
        self.set_returning(Returning::Expr(expr.into()));
    }

    /// Returns a reference to this statement's `RETURNING` clause.
    ///
    /// # Panics
    ///
    /// Panics if the statement does not have a `RETURNING` clause.
    #[track_caller]
    pub fn returning_unwrap(&self) -> &Returning {
        self.returning().unwrap_or_else(|| {
            panic!("expected statement to have RETURNING clause; actual={self:#?}")
        })
    }

    /// Returns a mutable reference to this statement's `RETURNING` clause, if present.
    ///
    /// Returns `None` if the statement does not have a `RETURNING` clause or is
    /// a statement type that does not support `RETURNING`.
    pub fn returning_mut(&mut self) -> Option<&mut Returning> {
        match self {
            Statement::Delete(delete) => delete.returning.as_mut(),
            Statement::Insert(insert) => insert.returning.as_mut(),
            Statement::Query(query) => query.returning_mut(),
            Statement::Update(update) => update.returning.as_mut(),
        }
    }

    /// Returns a mutable reference to this statement's `RETURNING` clause.
    ///
    /// # Panics
    ///
    /// Panics if the statement does not have a `RETURNING` clause.
    #[track_caller]
    pub fn returning_mut_unwrap(&mut self) -> &mut Returning {
        match self {
            Statement::Delete(delete) => delete.returning.as_mut().unwrap(),
            Statement::Insert(insert) => insert.returning.as_mut().unwrap(),
            Statement::Query(query) => query.returning_mut_unwrap(),
            Statement::Update(update) => update.returning.as_mut().unwrap(),
        }
    }
}

impl Query {
    /// Returns a reference to this query's `RETURNING` clause, if present.
    ///
    /// Returns `Some` only for `SELECT` queries. Other query types (`VALUES`,
    /// `UNION`, etc.) do not have a `RETURNING` clause.
    pub fn returning(&self) -> Option<&Returning> {
        match &self.body {
            stmt::ExprSet::Select(select) => Some(&select.returning),
            _ => None,
        }
    }

    /// Returns a reference to this query's `RETURNING` clause.
    ///
    /// # Panics
    ///
    /// Panics if the query does not have a `RETURNING` clause (i.e., the body
    /// is not a `SELECT`).
    #[track_caller]
    pub fn returning_unwrap(&self) -> &Returning {
        self.returning()
            .unwrap_or_else(|| panic!("expected query to have RETURNING clause; actual={self:#?}"))
    }

    /// Returns a mutable reference to this query's `RETURNING` clause, if present.
    ///
    /// Returns `Some` only for `SELECT` queries. Other query types (`VALUES`,
    /// `UNION`, etc.) do not have a `RETURNING` clause.
    pub fn returning_mut(&mut self) -> Option<&mut Returning> {
        match &mut self.body {
            stmt::ExprSet::Select(select) => Some(&mut select.returning),
            _ => None,
        }
    }

    /// Returns a mutable reference to this query's `RETURNING` clause.
    ///
    /// # Panics
    ///
    /// Panics if the query does not have a `RETURNING` clause (i.e., the body
    /// is not a `SELECT`).
    #[track_caller]
    pub fn returning_mut_unwrap(&mut self) -> &mut Returning {
        match &mut self.body {
            stmt::ExprSet::Select(select) => &mut select.returning,
            body => panic!("expected query to have RETURNING clause; actual={body:#?}"),
        }
    }
}

impl Node for Returning {
    fn visit<V: stmt::Visit>(&self, mut visit: V)
    where
        Self: Sized,
    {
        visit.visit_returning(self);
    }

    fn visit_mut<V: stmt::VisitMut>(&mut self, mut visit: V) {
        visit.visit_returning_mut(self);
    }
}
