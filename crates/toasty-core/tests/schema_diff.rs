use toasty_core::schema::db::{
    Column, ColumnId, ColumnsDiffItem, Index, IndexColumn, IndexId, IndexOp, IndexScope,
    IndicesDiffItem, PrimaryKey, RenameHints, Schema, SchemaDiff, Table, TableId, TablesDiffItem,
    Type,
};
use toasty_core::stmt;

fn col(table: usize, idx: usize, name: &str, ty: Type, nullable: bool) -> Column {
    Column {
        id: ColumnId {
            table: TableId(table),
            index: idx,
        },
        name: name.to_string(),
        ty: stmt::Type::String,
        storage_ty: ty,
        nullable,
        primary_key: idx == 0,
        auto_increment: false,
    }
}

// idx is the position in the table's `indices` Vec (used as the vec index by schema lookups)
fn make_index(table_id: usize, idx: usize, name: &str, on_col: usize, unique: bool) -> Index {
    Index {
        id: IndexId {
            table: TableId(table_id),
            index: idx,
        },
        name: name.to_string(),
        on: TableId(table_id),
        columns: vec![IndexColumn {
            column: ColumnId {
                table: TableId(table_id),
                index: on_col,
            },
            op: IndexOp::Eq,
            scope: IndexScope::Local,
        }],
        unique,
        primary_key: false,
    }
}

fn table(id: usize, name: &str, cols: Vec<Column>, indices: Vec<Index>) -> Table {
    let pk_cols: Vec<ColumnId> = cols
        .iter()
        .filter(|c| c.primary_key)
        .map(|c| c.id)
        .collect();
    Table {
        id: TableId(id),
        name: name.to_string(),
        columns: cols,
        primary_key: PrimaryKey {
            columns: pk_cols,
            index: IndexId {
                table: TableId(id),
                index: 0,
            },
        },
        indices,
    }
}

fn schema(tables: Vec<Table>) -> Schema {
    Schema { tables }
}

fn users_table() -> Table {
    table(
        0,
        "users",
        vec![
            col(0, 0, "id", Type::Integer(8), false),
            col(0, 1, "name", Type::Text, false),
        ],
        vec![],
    )
}

// ── Empty / no-change cases ──────────────────────────────────────────────────

#[test]
fn empty_schemas_no_diff() {
    let from = schema(vec![]);
    let to = schema(vec![]);
    let hints = RenameHints::new();
    let diff = SchemaDiff::from(&from, &to, &hints);
    assert!(diff.is_empty());
}

#[test]
fn same_schema_no_diff() {
    let s = schema(vec![users_table()]);
    let hints = RenameHints::new();
    let diff = SchemaDiff::from(&s, &s, &hints);
    assert!(diff.is_empty());
}

// ── Table-level changes ──────────────────────────────────────────────────────

#[test]
fn create_table() {
    let from = schema(vec![]);
    let to = schema(vec![users_table()]);
    let hints = RenameHints::new();
    let diff = SchemaDiff::from(&from, &to, &hints);

    assert!(!diff.is_empty());
    let created: Vec<_> = diff
        .tables()
        .iter()
        .filter(|item| matches!(item, TablesDiffItem::CreateTable(_)))
        .collect();
    assert_eq!(created.len(), 1);
    if let TablesDiffItem::CreateTable(t) = created[0] {
        assert_eq!(t.name, "users");
    }
}

#[test]
fn drop_table() {
    let from = schema(vec![users_table()]);
    let to = schema(vec![]);
    let hints = RenameHints::new();
    let diff = SchemaDiff::from(&from, &to, &hints);

    assert!(!diff.is_empty());
    let dropped: Vec<_> = diff
        .tables()
        .iter()
        .filter(|item| matches!(item, TablesDiffItem::DropTable(_)))
        .collect();
    assert_eq!(dropped.len(), 1);
    if let TablesDiffItem::DropTable(t) = dropped[0] {
        assert_eq!(t.name, "users");
    }
}

#[test]
fn no_hint_is_drop_and_create() {
    // Without rename hints, dropping "users" and adding "people" = DropTable + CreateTable
    let from = schema(vec![users_table()]);
    let to = schema(vec![table(
        0,
        "people",
        vec![
            col(0, 0, "id", Type::Integer(8), false),
            col(0, 1, "name", Type::Text, false),
        ],
        vec![],
    )]);
    let hints = RenameHints::new();
    let diff = SchemaDiff::from(&from, &to, &hints);

    assert!(!diff.is_empty());
    let items: Vec<_> = diff.tables().iter().collect();
    assert!(items
        .iter()
        .any(|i| matches!(i, TablesDiffItem::DropTable(t) if t.name == "users")));
    assert!(items
        .iter()
        .any(|i| matches!(i, TablesDiffItem::CreateTable(t) if t.name == "people")));
}

#[test]
fn rename_table_with_hint() {
    let from_table = table(
        0,
        "users",
        vec![col(0, 0, "id", Type::Integer(8), false)],
        vec![],
    );
    let to_table = table(
        0,
        "people",
        vec![col(0, 0, "id", Type::Integer(8), false)],
        vec![],
    );
    let from = schema(vec![from_table]);
    let to = schema(vec![to_table]);

    let mut hints = RenameHints::new();
    hints.add_table_hint(TableId(0), TableId(0));

    let diff = SchemaDiff::from(&from, &to, &hints);

    // Should be AlterTable (rename), not DropTable + CreateTable
    let items: Vec<_> = diff.tables().iter().collect();
    assert_eq!(items.len(), 1, "expected exactly one diff item");
    assert!(
        matches!(
            items[0],
            TablesDiffItem::AlterTable {
                previous,
                next,
                ..
            } if previous.name == "users" && next.name == "people"
        ),
        "expected AlterTable(users -> people), got something else"
    );
}

// ── Column-level changes ─────────────────────────────────────────────────────

#[test]
fn add_column() {
    let from = schema(vec![table(
        0,
        "users",
        vec![col(0, 0, "id", Type::Integer(8), false)],
        vec![],
    )]);
    let to = schema(vec![table(
        0,
        "users",
        vec![
            col(0, 0, "id", Type::Integer(8), false),
            col(0, 1, "email", Type::Text, false),
        ],
        vec![],
    )]);
    let hints = RenameHints::new();
    let diff = SchemaDiff::from(&from, &to, &hints);

    assert!(!diff.is_empty());
    let alter = diff
        .tables()
        .iter()
        .find_map(|i| {
            if let TablesDiffItem::AlterTable { columns, .. } = i {
                Some(columns)
            } else {
                None
            }
        })
        .expect("expected AlterTable");

    let added: Vec<_> = alter
        .iter()
        .filter(|c| matches!(c, ColumnsDiffItem::AddColumn(_)))
        .collect();
    assert_eq!(added.len(), 1);
    if let ColumnsDiffItem::AddColumn(c) = added[0] {
        assert_eq!(c.name, "email");
    }
}

#[test]
fn drop_column() {
    let from = schema(vec![table(
        0,
        "users",
        vec![
            col(0, 0, "id", Type::Integer(8), false),
            col(0, 1, "email", Type::Text, false),
        ],
        vec![],
    )]);
    let to = schema(vec![table(
        0,
        "users",
        vec![col(0, 0, "id", Type::Integer(8), false)],
        vec![],
    )]);
    let hints = RenameHints::new();
    let diff = SchemaDiff::from(&from, &to, &hints);

    assert!(!diff.is_empty());
    let alter = diff
        .tables()
        .iter()
        .find_map(|i| {
            if let TablesDiffItem::AlterTable { columns, .. } = i {
                Some(columns)
            } else {
                None
            }
        })
        .expect("expected AlterTable");

    let dropped: Vec<_> = alter
        .iter()
        .filter(|c| matches!(c, ColumnsDiffItem::DropColumn(_)))
        .collect();
    assert_eq!(dropped.len(), 1);
    if let ColumnsDiffItem::DropColumn(c) = dropped[0] {
        assert_eq!(c.name, "email");
    }
}

#[test]
fn rename_column_without_hint_is_drop_and_add() {
    let from = schema(vec![table(
        0,
        "users",
        vec![
            col(0, 0, "id", Type::Integer(8), false),
            col(0, 1, "old_name", Type::Text, false),
        ],
        vec![],
    )]);
    let to = schema(vec![table(
        0,
        "users",
        vec![
            col(0, 0, "id", Type::Integer(8), false),
            col(0, 1, "new_name", Type::Text, false),
        ],
        vec![],
    )]);
    let hints = RenameHints::new();
    let diff = SchemaDiff::from(&from, &to, &hints);

    assert!(!diff.is_empty());
    let alter = diff
        .tables()
        .iter()
        .find_map(|i| {
            if let TablesDiffItem::AlterTable { columns, .. } = i {
                Some(columns)
            } else {
                None
            }
        })
        .expect("expected AlterTable");

    let col_items: Vec<_> = alter.iter().collect();
    assert!(col_items
        .iter()
        .any(|c| matches!(c, ColumnsDiffItem::DropColumn(col) if col.name == "old_name")));
    assert!(col_items
        .iter()
        .any(|c| matches!(c, ColumnsDiffItem::AddColumn(col) if col.name == "new_name")));
}

#[test]
fn rename_column_with_hint() {
    let from = schema(vec![table(
        0,
        "users",
        vec![
            col(0, 0, "id", Type::Integer(8), false),
            col(0, 1, "old_name", Type::Text, false),
        ],
        vec![],
    )]);
    let to = schema(vec![table(
        0,
        "users",
        vec![
            col(0, 0, "id", Type::Integer(8), false),
            col(0, 1, "new_name", Type::Text, false),
        ],
        vec![],
    )]);

    let mut hints = RenameHints::new();
    hints.add_column_hint(
        ColumnId {
            table: TableId(0),
            index: 1,
        },
        ColumnId {
            table: TableId(0),
            index: 1,
        },
    );

    let diff = SchemaDiff::from(&from, &to, &hints);

    assert!(!diff.is_empty());
    let alter = diff
        .tables()
        .iter()
        .find_map(|i| {
            if let TablesDiffItem::AlterTable { columns, .. } = i {
                Some(columns)
            } else {
                None
            }
        })
        .expect("expected AlterTable");

    // With hint, should be AlterColumn, not DropColumn+AddColumn
    assert!(alter
        .iter()
        .any(|c| matches!(c, ColumnsDiffItem::AlterColumn { .. })));
    assert!(!alter
        .iter()
        .any(|c| matches!(c, ColumnsDiffItem::DropColumn(_))));
    assert!(!alter
        .iter()
        .any(|c| matches!(c, ColumnsDiffItem::AddColumn(_))));
}

#[test]
fn alter_column_nullability() {
    let from = schema(vec![table(
        0,
        "users",
        vec![
            col(0, 0, "id", Type::Integer(8), false),
            col(0, 1, "email", Type::Text, false),
        ],
        vec![],
    )]);
    // Make the email column nullable
    let mut nullable_email = col(0, 1, "email", Type::Text, true);
    nullable_email.nullable = true;
    let to = schema(vec![table(
        0,
        "users",
        vec![col(0, 0, "id", Type::Integer(8), false), nullable_email],
        vec![],
    )]);
    let hints = RenameHints::new();
    let diff = SchemaDiff::from(&from, &to, &hints);

    assert!(!diff.is_empty());
    let alter = diff
        .tables()
        .iter()
        .find_map(|i| {
            if let TablesDiffItem::AlterTable { columns, .. } = i {
                Some(columns)
            } else {
                None
            }
        })
        .expect("expected AlterTable");

    assert!(alter
        .iter()
        .any(|c| matches!(c, ColumnsDiffItem::AlterColumn { .. })));
}

// ── Index-level changes ───────────────────────────────────────────────────────

#[test]
fn create_index() {
    let from = schema(vec![table(
        0,
        "users",
        vec![
            col(0, 0, "id", Type::Integer(8), false),
            col(0, 1, "email", Type::Text, false),
        ],
        vec![],
    )]);
    let to = schema(vec![table(
        0,
        "users",
        vec![
            col(0, 0, "id", Type::Integer(8), false),
            col(0, 1, "email", Type::Text, false),
        ],
        vec![make_index(0, 0, "idx_users_email", 1, false)],
    )]);
    let hints = RenameHints::new();
    let diff = SchemaDiff::from(&from, &to, &hints);

    assert!(!diff.is_empty());
    let indices = diff
        .tables()
        .iter()
        .find_map(|i| {
            if let TablesDiffItem::AlterTable { indices, .. } = i {
                Some(indices)
            } else {
                None
            }
        })
        .expect("expected AlterTable");

    assert!(indices
        .iter()
        .any(|i| matches!(i, IndicesDiffItem::CreateIndex(_))));
}

#[test]
fn drop_index() {
    let from = schema(vec![table(
        0,
        "users",
        vec![
            col(0, 0, "id", Type::Integer(8), false),
            col(0, 1, "email", Type::Text, false),
        ],
        vec![make_index(0, 0, "idx_users_email", 1, false)],
    )]);
    let to = schema(vec![table(
        0,
        "users",
        vec![
            col(0, 0, "id", Type::Integer(8), false),
            col(0, 1, "email", Type::Text, false),
        ],
        vec![],
    )]);
    let hints = RenameHints::new();
    let diff = SchemaDiff::from(&from, &to, &hints);

    assert!(!diff.is_empty());
    let indices = diff
        .tables()
        .iter()
        .find_map(|i| {
            if let TablesDiffItem::AlterTable { indices, .. } = i {
                Some(indices)
            } else {
                None
            }
        })
        .expect("expected AlterTable");

    assert!(indices
        .iter()
        .any(|i| matches!(i, IndicesDiffItem::DropIndex(_))));
}

#[test]
fn rename_index_with_hint() {
    let from = schema(vec![table(
        0,
        "users",
        vec![
            col(0, 0, "id", Type::Integer(8), false),
            col(0, 1, "email", Type::Text, false),
        ],
        vec![make_index(0, 0, "idx_old", 1, false)],
    )]);
    let to = schema(vec![table(
        0,
        "users",
        vec![
            col(0, 0, "id", Type::Integer(8), false),
            col(0, 1, "email", Type::Text, false),
        ],
        vec![make_index(0, 0, "idx_new", 1, false)],
    )]);

    let mut hints = RenameHints::new();
    hints.add_index_hint(
        IndexId {
            table: TableId(0),
            index: 0,
        },
        IndexId {
            table: TableId(0),
            index: 0,
        },
    );

    let diff = SchemaDiff::from(&from, &to, &hints);

    assert!(!diff.is_empty());
    let indices = diff
        .tables()
        .iter()
        .find_map(|i| {
            if let TablesDiffItem::AlterTable { indices, .. } = i {
                Some(indices)
            } else {
                None
            }
        })
        .expect("expected AlterTable");

    assert!(indices
        .iter()
        .any(|i| matches!(i, IndicesDiffItem::AlterIndex { .. })));
    assert!(!indices
        .iter()
        .any(|i| matches!(i, IndicesDiffItem::DropIndex(_))));
    assert!(!indices
        .iter()
        .any(|i| matches!(i, IndicesDiffItem::CreateIndex(_))));
}

// ── Multi-table changes ───────────────────────────────────────────────────────

#[test]
fn multiple_tables_partial_change() {
    // users unchanged, todos dropped, posts added
    let users = table(
        0,
        "users",
        vec![col(0, 0, "id", Type::Integer(8), false)],
        vec![],
    );
    let todos = table(
        1,
        "todos",
        vec![col(1, 0, "id", Type::Integer(8), false)],
        vec![],
    );
    // posts is at position 1 in the `to` vec; TableId must match its vec index
    let posts = table(
        1,
        "posts",
        vec![col(1, 0, "id", Type::Integer(8), false)],
        vec![],
    );

    let from = schema(vec![users.clone(), todos]);
    let to = schema(vec![users, posts]);

    let hints = RenameHints::new();
    let diff = SchemaDiff::from(&from, &to, &hints);

    assert!(!diff.is_empty());
    let items: Vec<_> = diff.tables().iter().collect();

    assert!(items
        .iter()
        .any(|i| matches!(i, TablesDiffItem::DropTable(t) if t.name == "todos")));
    assert!(items
        .iter()
        .any(|i| matches!(i, TablesDiffItem::CreateTable(t) if t.name == "posts")));
    // users should not appear in the diff
    assert!(!items.iter().any(|i| {
        matches!(i, TablesDiffItem::DropTable(t) | TablesDiffItem::CreateTable(t) if t.name == "users")
    }));
}
