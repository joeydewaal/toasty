use super::{Expr, Include};
use crate::stmt::{self, ExprSet, Node, Query, Statement, Visit};

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
/// let ret = Returning::model();
/// assert!(ret.is_model());
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct Returning {
    /// The value returned for each affected row.
    pub expr: ReturningExpr,

    /// Whether the statement returns a list or at most one value.
    pub cardinality: ReturningCardinality,
}

/// The value produced by a returning clause.
#[derive(Debug, Clone, PartialEq)]
pub enum ReturningExpr {
    /// Return the full model with the specified association includes.
    Model {
        /// Which mutation row image supplies the model fields.
        image: RowImage,

        /// Associations that should be eagerly loaded, with optional
        /// per-relation filters.
        include: Vec<Include>,
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

/// Selects values before or after a mutation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RowImage {
    /// Values before the mutation.
    Old,

    /// Values after the mutation.
    New,
}

/// Controls the number of values produced by a returning clause.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum ReturningCardinality {
    /// Return every projected value in a list.
    #[default]
    List,

    /// Return at most one projected value.
    Single,
}

impl Returning {
    /// Creates a returning clause with list cardinality.
    pub fn new(expr: ReturningExpr) -> Self {
        Self {
            expr,
            cardinality: ReturningCardinality::List,
        }
    }

    /// Returns the current model without association includes.
    pub fn model() -> Self {
        Self::new(ReturningExpr::Model {
            image: RowImage::New,
            include: vec![],
        })
    }

    /// Returns the model's pre-mutation values without association includes.
    pub fn old_model() -> Self {
        Self::new(ReturningExpr::Model {
            image: RowImage::Old,
            include: vec![],
        })
    }

    /// Changes the output cardinality to at most one value.
    pub fn single(mut self) -> Self {
        self.cardinality = ReturningCardinality::Single;
        self
    }

    /// Returns `true` when this clause produces at most one value.
    pub fn is_single(&self) -> bool {
        self.cardinality == ReturningCardinality::Single
    }

    /// Returns a projection evaluated against each source row.
    pub fn project(expr: Expr) -> Self {
        Self::new(ReturningExpr::Project(expr))
    }

    /// Returns an expression independent of the statement source.
    pub fn expression(expr: Expr) -> Self {
        Self::new(ReturningExpr::Expr(expr))
    }

    /// Returns whether the mutation changed any rows.
    pub fn changed() -> Self {
        Self::new(ReturningExpr::Changed)
    }

    /// Returns the number of rows affected by the mutation.
    pub fn count() -> Self {
        Self::new(ReturningExpr::Count)
    }

    /// Creates a `ReturningExpr::Project` from an iterator of expressions, combining
    /// them into a record expression.
    pub fn from_project_iter<T>(items: impl IntoIterator<Item = T>) -> Self
    where
        T: Into<Expr>,
    {
        Returning::project(Expr::record(items))
    }

    /// Returns `true` if this returns a current or pre-update model.
    pub fn is_model(&self) -> bool {
        matches!(self.expr, ReturningExpr::Model { .. })
    }

    /// Returns the association includes for current or old models.
    pub fn model_includes(&self) -> &[Include] {
        match &self.expr {
            ReturningExpr::Model { include, .. } => include,
            _ => &[],
        }
    }

    /// Returns a mutable reference to a model variant's includes.
    ///
    /// # Panics
    ///
    /// Panics if this is not `Model`.
    #[track_caller]
    pub fn model_includes_mut_unwrap(&mut self) -> &mut Vec<Include> {
        match &mut self.expr {
            ReturningExpr::Model { include, .. } => include,
            _ => panic!("not a Model variant"),
        }
    }

    /// Returns `true` if this is the `Changed` variant.
    pub fn is_changed(&self) -> bool {
        matches!(self.expr, ReturningExpr::Changed)
    }

    /// Returns `true` if this is the `Count` variant.
    pub fn is_count(&self) -> bool {
        matches!(self.expr, ReturningExpr::Count)
    }

    /// Returns `true` if this is the `Project` variant.
    pub fn is_project(&self) -> bool {
        matches!(self.expr, ReturningExpr::Project(_))
    }

    /// Returns a reference to the inner expression if this is the `Project`
    /// variant.
    pub fn as_project(&self) -> Option<&Expr> {
        match &self.expr {
            ReturningExpr::Project(expr) => Some(expr),
            _ => None,
        }
    }

    /// Returns `true` when this clause reads pre-mutation values.
    pub fn uses_old(&self) -> bool {
        if matches!(
            self.expr,
            ReturningExpr::Model {
                image: RowImage::Old,
                ..
            }
        ) {
            return true;
        }

        struct FindOld(bool);

        impl Visit for FindOld {
            fn visit_expr_row(&mut self, expr: &stmt::ExprRow) {
                self.0 |= matches!(expr, stmt::ExprRow::Old(_));
            }
        }

        let Some(expr) = self.as_project() else {
            return false;
        };
        let mut find = FindOld(false);
        find.visit_expr(expr);
        find.0
    }

    /// Returns `true` when this clause reads post-mutation values.
    pub fn uses_new(&self) -> bool {
        if matches!(
            self.expr,
            ReturningExpr::Model {
                image: RowImage::New,
                ..
            }
        ) {
            return true;
        }

        struct FindReference(bool);

        impl Visit for FindReference {
            fn visit_expr_reference(&mut self, _expr: &stmt::ExprReference) {
                self.0 = true;
            }
        }

        let Some(expr) = self.as_project() else {
            return false;
        };
        let mut find = FindReference(false);
        find.visit_expr(expr);
        find.0
    }

    /// Returns a reference to the inner expression.
    ///
    /// # Panics
    ///
    /// Panics if this is not the `Project` variant.
    #[track_caller]
    pub fn as_project_unwrap(&self) -> &Expr {
        self.as_project()
            .unwrap_or_else(|| panic!("expected stmt::ReturningExpr::Project; actual={self:#?}"))
    }

    /// Returns a mutable reference to the inner expression if this is the
    /// `Project` variant.
    pub fn as_project_mut(&mut self) -> Option<&mut Expr> {
        match &mut self.expr {
            ReturningExpr::Project(expr) => Some(expr),
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
        match &mut self.expr {
            ReturningExpr::Project(expr) => expr,
            _ => panic!("expected stmt::ReturningExpr::Project"),
        }
    }

    /// Replaces this returning clause with `ReturningExpr::Project` containing the
    /// given expression.
    pub fn set_project(&mut self, expr: impl Into<Expr>) {
        let cardinality = self.cardinality;
        *self = Returning::project(expr.into());
        self.cardinality = cardinality;
    }

    /// Returns `true` if this is the `Expr` variant.
    pub fn is_expr(&self) -> bool {
        matches!(self.expr, ReturningExpr::Expr(..))
    }

    /// Takes this returning clause, replaces it with a null projection, and
    /// returns the original value.
    pub fn take(&mut self) -> Returning {
        let cardinality = self.cardinality;
        std::mem::replace(
            self,
            Returning {
                expr: ReturningExpr::Project(stmt::Expr::null()),
                cardinality,
            },
        )
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

    /// Set the `Returning` clause to `ReturningExpr::Project` containing the given
    /// expression.
    pub fn set_returning_project(&mut self, expr: impl Into<Expr>) {
        self.set_returning(Returning::project(expr.into()));
    }

    /// Set the `Returning` clause to `ReturningExpr::Expr` containing the given
    /// expression.
    pub fn set_returning_expr(&mut self, expr: impl Into<Expr>) {
        self.set_returning(Returning::expression(expr.into()));
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
