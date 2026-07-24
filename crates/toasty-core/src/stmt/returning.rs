use super::{Expr, Include};
use crate::stmt::{self, ExprSet, Node, Query, Statement};

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
/// let ret = Returning::Model { include: vec![] };
/// assert!(ret.is_model());
/// ```
#[derive(Debug, Clone, PartialEq)]
pub enum Returning {
    /// Return the full model with the specified association includes.
    Model {
        /// Associations that should be eagerly loaded, with optional
        /// per-relation filters.
        include: Vec<Include>,
    },

    /// Return the model without implicitly loading relation fields.
    ModelUnloaded {
        /// Explicit relation includes. Usually empty for mutation results.
        include: Vec<Include>,
    },

    /// Return at most the first row, using the query's ordering when present.
    First {
        /// The values to return.
        returning: Box<Returning>,

        /// Query selecting the first pre-update primary key.
        selector: Option<Expr>,

        /// Primary-key field positions in the returned model.
        key: Vec<usize>,
    },

    /// Return one row and fail when the update matched no records.
    One {
        /// The values to return.
        returning: Box<Returning>,

        /// Query selecting the first pre-update primary key.
        selector: Option<Expr>,

        /// Primary-key field positions in the returned model.
        key: Vec<usize>,
    },

    /// Return whether the operation changed any rows.
    Changed,

    /// Return the number of rows affected by a mutation.
    Count,

    /// Return the result of evaluating an expression against the source rows.
    Project(Expr),

    /// Return a fixed expression, independent of the statement source.
    Expr(Expr),

    /// Return values from before the mutation instead of after it.
    Old(Box<Returning>),
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
        match self {
            Self::Model { .. } | Self::ModelUnloaded { .. } => true,
            Self::First { returning, .. } | Self::One { returning, .. } | Self::Old(returning) => {
                returning.is_model()
            }
            _ => false,
        }
    }

    /// Returns the association includes for a `Model` variant, or an
    /// empty slice for other variants.
    pub fn model_includes(&self) -> &[Include] {
        match self {
            Self::Model { include } | Self::ModelUnloaded { include } => include,
            Self::First { returning, .. } | Self::One { returning, .. } | Self::Old(returning) => {
                returning.model_includes()
            }
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
            Self::Model { include } | Self::ModelUnloaded { include } => include,
            Self::First { returning, .. } | Self::One { returning, .. } | Self::Old(returning) => {
                returning.model_includes_mut_unwrap()
            }
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
        match self {
            Self::Project(_) => true,
            Self::First { returning, .. } | Self::One { returning, .. } | Self::Old(returning) => {
                returning.is_project()
            }
            _ => false,
        }
    }

    /// Returns a reference to the inner expression if this is the `Project`
    /// variant.
    pub fn as_project(&self) -> Option<&Expr> {
        match self {
            Self::Project(expr) => Some(expr),
            Self::First { returning, .. } | Self::One { returning, .. } | Self::Old(returning) => {
                returning.as_project()
            }
            _ => None,
        }
    }

    /// Returns `true` when this clause requests pre-mutation values.
    pub fn is_old(&self) -> bool {
        match self {
            Self::Old(_) => true,
            Self::First { returning, .. } | Self::One { returning, .. } => returning.is_old(),
            _ => false,
        }
    }

    /// Select pre-mutation values.
    pub fn into_old(self) -> Self {
        match self {
            Self::Old(_) => self,
            returning => Self::Old(Box::new(returning)),
        }
    }

    /// Select post-mutation values.
    pub fn into_new(self) -> Self {
        match self {
            Self::Old(returning) => *returning,
            Self::First {
                returning,
                selector,
                key,
            } => Self::First {
                returning: Box::new(returning.into_new()),
                selector,
                key,
            },
            Self::One {
                returning,
                selector,
                key,
            } => Self::One {
                returning: Box::new(returning.into_new()),
                selector,
                key,
            },
            returning => returning,
        }
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
            Self::First { returning, .. } | Self::One { returning, .. } | Self::Old(returning) => {
                returning.as_project_mut()
            }
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
        match self {
            Self::First { returning, .. } | Self::One { returning, .. } | Self::Old(returning) => {
                returning.set_project(expr)
            }
            returning => *returning = Returning::Project(expr.into()),
        }
    }

    /// Returns `true` if this is the `Expr` variant.
    pub fn is_expr(&self) -> bool {
        match self {
            Self::Expr(_) => true,
            Self::First { returning, .. } | Self::One { returning, .. } | Self::Old(returning) => {
                returning.is_expr()
            }
            _ => false,
        }
    }

    /// Attach a query that selects the first pre-update row.
    pub fn set_selector(&mut self, selector: Option<Expr>) {
        match self {
            Self::First {
                selector: target, ..
            }
            | Self::One {
                selector: target, ..
            } => *target = selector,
            Self::Old(returning) => returning.set_selector(selector),
            _ => {}
        }
    }

    /// Remove result cardinality metadata, leaving the underlying projection.
    pub fn take_rows(&mut self) -> ReturningRows {
        let returning = self.take();
        match returning {
            Self::First {
                returning,
                selector,
                key,
            } => {
                *self = *returning;
                ReturningRows::First { selector, key }
            }
            Self::One {
                returning,
                selector,
                key,
            } => {
                *self = *returning;
                ReturningRows::One { selector, key }
            }
            returning => {
                *self = returning;
                ReturningRows::All
            }
        }
    }

    /// Takes this returning clause, replacing it with `Returning::Project(null)`,
    /// and returns the original value.
    pub fn take(&mut self) -> Returning {
        std::mem::replace(self, Returning::Project(stmt::Expr::null()))
    }
}

/// Cardinality shaping applied to rows returned from a mutation.
#[derive(Debug, Clone, PartialEq)]
pub enum ReturningRows {
    /// Return every row.
    All,

    /// Return at most the first row.
    First {
        /// Query selecting the first pre-update primary key.
        selector: Option<Expr>,

        /// Primary-key field positions in the returned model.
        key: Vec<usize>,
    },

    /// Return one row and fail when none matched.
    One {
        /// Query selecting the first pre-update primary key.
        selector: Option<Expr>,

        /// Primary-key field positions in the returned model.
        key: Vec<usize>,
    },
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
