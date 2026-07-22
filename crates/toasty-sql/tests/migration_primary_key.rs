use toasty_core::{
    driver::Capability,
    schema::{
        db::{
            Column, ColumnId, Index, IndexColumn, IndexId, IndexOp, IndexScope, PrimaryKey, Schema,
            Table, TableId, Type,
        },
        diff,
    },
    stmt,
};
use toasty_sql::{Serializer, migration::MigrationStatement};

fn make_column(index: usize, name: &str, primary_key: bool, auto_increment: bool) -> Column {
    Column {
        id: ColumnId {
            table: TableId(0),
            index,
        },
        name: name.to_string(),
        ty: stmt::Type::U64,
        storage_ty: Type::Integer(8),
        nullable: false,
        primary_key,
        auto_increment,
        versionable: false,
    }
}

fn make_index(
    index: usize,
    name: &str,
    columns: &[usize],
    unique: bool,
    primary_key: bool,
) -> Index {
    Index {
        id: IndexId {
            table: TableId(0),
            index,
        },
        name: name.to_string(),
        on: TableId(0),
        columns: columns
            .iter()
            .map(|&index| IndexColumn {
                column: ColumnId {
                    table: TableId(0),
                    index,
                },
                op: IndexOp::Eq,
                scope: IndexScope::Local,
            })
            .collect(),
        unique,
        primary_key,
    }
}

fn make_table(columns: Vec<Column>, primary_key: &[usize], indices: Vec<Index>) -> Table {
    Table {
        id: TableId(0),
        name: "items".to_string(),
        columns,
        primary_key: PrimaryKey {
            columns: primary_key
                .iter()
                .map(|&index| ColumnId {
                    table: TableId(0),
                    index,
                })
                .collect(),
            index: IndexId {
                table: TableId(0),
                index: 0,
            },
        },
        indices,
    }
}

fn serialize_migration(statements: &[MigrationStatement<'_>]) -> Vec<String> {
    statements
        .iter()
        .map(|statement| Serializer::sqlite(statement.schema()).serialize(statement.statement()))
        .collect()
}

#[test]
fn change_primary_key_sqlite_recreates_table() {
    let previous = Schema {
        tables: vec![make_table(
            vec![
                make_column(0, "id", true, true),
                make_column(1, "foo_id", false, false),
                make_column(2, "bar_id", false, false),
            ],
            &[0],
            vec![
                make_index(0, "index_items_by_id", &[0], true, true),
                make_index(1, "index_items_by_foo_id", &[1], false, false),
                make_index(2, "index_items_by_bar_id", &[2], false, false),
            ],
        )],
    };
    let next = Schema {
        tables: vec![make_table(
            vec![
                make_column(0, "foo_id", true, false),
                make_column(1, "bar_id", true, false),
            ],
            &[0, 1],
            vec![
                make_index(0, "index_items_by_foo_id_and_bar_id", &[0, 1], true, true),
                make_index(1, "index_items_by_foo_id", &[0], false, false),
                make_index(2, "index_items_by_bar_id", &[1], false, false),
            ],
        )],
    };

    let hints = diff::RenameHints::new();
    let schema_diff = diff::Schema::from(&previous, &next, &hints);
    let statements = MigrationStatement::from_diff(&schema_diff, &Capability::SQLITE);
    let sql = serialize_migration(&statements);

    assert_eq!(
        sql,
        [
            "PRAGMA foreign_keys = OFF;",
            "CREATE TABLE \"_toasty_new_items\" (\n    \"foo_id\" BIGINT NOT NULL,\n    \"bar_id\" BIGINT NOT NULL,\n    PRIMARY KEY (\"foo_id\", \"bar_id\")\n);",
            "INSERT INTO \"_toasty_new_items\" (\"foo_id\", \"bar_id\") SELECT \"foo_id\", \"bar_id\" FROM \"items\";",
            "DROP TABLE \"items\";",
            "ALTER TABLE \"_toasty_new_items\" RENAME TO \"items\";",
            "PRAGMA foreign_keys = ON;",
            "CREATE INDEX \"index_items_by_foo_id\" ON \"items\" (\"foo_id\");",
            "CREATE INDEX \"index_items_by_bar_id\" ON \"items\" (\"bar_id\");",
        ]
    );
}
