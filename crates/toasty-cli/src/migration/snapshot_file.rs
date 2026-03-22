use anyhow::{Result, bail};
use serde::{Deserialize, Serialize};
use std::fmt;
use std::path::Path;
use std::str::FromStr;
use toasty::schema::db::{ColumnId, IndexId, RenameHints, Schema, TableId};
use toml_edit::{ArrayOfTables, DocumentMut, Item, Value};

const SNAPSHOT_FILE_VERSION: u32 = 1;

/// Rename hints stored in a snapshot file.
///
/// Parallel to [`RenameHints`] but uses plain integer indices instead of
/// the newtype wrappers so that the data serializes cleanly to TOML.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct StoredRenameHints {
    /// Table renames: `[from_table_index, to_table_index]`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tables: Vec<StoredTableRename>,

    /// Column renames.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub columns: Vec<StoredColumnRename>,

    /// Index renames.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub indices: Vec<StoredIndexRename>,
}

/// A single table rename recorded in a snapshot.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredTableRename {
    pub from: usize,
    pub to: usize,
}

/// A single column rename recorded in a snapshot.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredColumnRename {
    pub from_table: usize,
    pub from_col: usize,
    pub to_table: usize,
    pub to_col: usize,
}

/// A single index rename recorded in a snapshot.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredIndexRename {
    pub from_table: usize,
    pub from_idx: usize,
    pub to_table: usize,
    pub to_idx: usize,
}

impl StoredRenameHints {
    /// Returns `true` if there are no rename hints.
    pub fn is_empty(&self) -> bool {
        self.tables.is_empty() && self.columns.is_empty() && self.indices.is_empty()
    }

    /// Converts stored hints back into [`RenameHints`] for use during diff computation.
    pub fn into_rename_hints(self) -> RenameHints {
        let mut hints = RenameHints::new();
        for r in self.tables {
            hints.add_table_hint(TableId(r.from), TableId(r.to));
        }
        for r in self.columns {
            hints.add_column_hint(
                ColumnId {
                    table: TableId(r.from_table),
                    index: r.from_col,
                },
                ColumnId {
                    table: TableId(r.to_table),
                    index: r.to_col,
                },
            );
        }
        for r in self.indices {
            hints.add_index_hint(
                IndexId {
                    table: TableId(r.from_table),
                    index: r.from_idx,
                },
                IndexId {
                    table: TableId(r.to_table),
                    index: r.to_idx,
                },
            );
        }
        hints
    }
}

/// Snapshot file containing the current database schema state
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnapshotFile {
    /// Snapshot file format version
    version: u32,

    /// Rename hints that were used to compute the diff from the previous snapshot.
    ///
    /// These allow the migration to be applied against any supported database by
    /// reconstructing the [`SchemaDiff`] at apply time rather than storing
    /// database-specific SQL.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rename_hints: Option<StoredRenameHints>,

    /// The database schema
    pub schema: Schema,
}

impl SnapshotFile {
    /// Create a new snapshot file with the given schema and no rename hints.
    pub fn new(schema: Schema) -> Self {
        Self {
            version: SNAPSHOT_FILE_VERSION,
            rename_hints: None,
            schema,
        }
    }

    /// Create a new snapshot file with the given schema and rename hints.
    pub fn with_rename_hints(schema: Schema, rename_hints: StoredRenameHints) -> Self {
        Self {
            version: SNAPSHOT_FILE_VERSION,
            rename_hints: Some(rename_hints),
            schema,
        }
    }

    /// Load a snapshot file from a TOML file
    pub fn load(path: impl AsRef<Path>) -> Result<Self> {
        let contents = std::fs::read_to_string(path.as_ref())?;
        contents.parse()
    }

    /// Save the snapshot file to a TOML file
    pub fn save(&self, path: impl AsRef<Path>) -> Result<()> {
        std::fs::write(path.as_ref(), self.to_string())?;
        Ok(())
    }
}

impl FromStr for SnapshotFile {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self> {
        let file: SnapshotFile = toml::from_str(s)?;

        // Validate version
        if file.version != SNAPSHOT_FILE_VERSION {
            bail!(
                "Unsupported snapshot file version: {}. Expected version {}",
                file.version,
                SNAPSHOT_FILE_VERSION
            );
        }

        Ok(file)
    }
}

impl fmt::Display for SnapshotFile {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let doc = self.to_toml_document().map_err(|_| fmt::Error)?;
        write!(f, "{}", doc)
    }
}

impl SnapshotFile {
    fn to_toml_document(&self) -> Result<DocumentMut> {
        let mut doc = toml_edit::ser::to_document(self)?;
        for (_key, item) in doc.as_table_mut().iter_mut() {
            expand_item(item, 1);
        }
        Ok(doc)
    }
}

/// Recursively expands inline TOML structures into pretty array-of-tables format.
///
/// `toml_edit::ser::to_document` serializes all structs as inline tables and all
/// `Vec<Struct>` as inline arrays. This function walks the tree and expands:
///
/// - Inline tables at depth 1 (e.g. the `[schema]` section) → `Table`
/// - Inline arrays of inline tables at depth ≤ 3 → `[[array.of.tables]]`
///
/// This matches the expected snapshot format:
///   `[[schema.tables]]`, `[[schema.tables.columns]]`, `[[schema.tables.indices]]`
///
/// Deeper structures (e.g. `id = { table = 0, index = 0 }`) stay inline.
fn expand_item(item: &mut Item, depth: usize) {
    // Expand top-level inline tables (e.g. `schema = { ... }`) into `[schema]` sections.
    if depth == 1 && matches!(item, Item::Value(Value::InlineTable(_))) {
        let mut placeholder = Item::None;
        std::mem::swap(item, &mut placeholder);
        if let Item::Value(Value::InlineTable(t)) = placeholder {
            let mut table = t.into_table();
            for (_, child) in table.iter_mut() {
                expand_item(child, depth + 1);
            }
            *item = Item::Table(table);
        }
        return;
    }

    // Expand inline arrays of inline tables into `[[array.of.tables]]` sections.
    if depth <= 3 {
        let should_expand = matches!(item, Item::Value(Value::Array(_))) && {
            if let Item::Value(Value::Array(arr)) = &*item {
                !arr.is_empty() && arr.iter().all(|v| matches!(v, Value::InlineTable(_)))
            } else {
                false
            }
        };

        if should_expand {
            let values: Vec<Value> = if let Item::Value(Value::Array(arr)) = &*item {
                arr.iter().cloned().collect()
            } else {
                vec![]
            };

            let mut aot = ArrayOfTables::new();
            for val in values {
                if let Value::InlineTable(t) = val {
                    let mut table = t.into_table();
                    for (_, child) in table.iter_mut() {
                        expand_item(child, depth + 1);
                    }
                    aot.push(table);
                }
            }
            *item = Item::ArrayOfTables(aot);
            return;
        }
    }

    // Recurse into already-expanded structures.
    match item {
        Item::Table(t) => {
            for (_, child) in t.iter_mut() {
                expand_item(child, depth + 1);
            }
        }
        Item::ArrayOfTables(aot) => {
            for table in aot.iter_mut() {
                for (_, child) in table.iter_mut() {
                    expand_item(child, depth + 1);
                }
            }
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use toasty::schema::db::Schema;

    #[test]
    fn snapshot_roundtrip_no_hints() {
        let snapshot = SnapshotFile::new(Schema::default());
        let toml_str = snapshot.to_string();

        assert!(
            !toml_str.contains("rename_hints"),
            "empty hints should be omitted from TOML"
        );

        let parsed: SnapshotFile = toml_str.parse().unwrap();
        assert!(parsed.rename_hints.is_none());
    }

    #[test]
    fn snapshot_roundtrip_with_hints() {
        let hints = StoredRenameHints {
            tables: vec![StoredTableRename { from: 0, to: 1 }],
            columns: vec![StoredColumnRename {
                from_table: 0,
                from_col: 1,
                to_table: 1,
                to_col: 2,
            }],
            indices: vec![StoredIndexRename {
                from_table: 0,
                from_idx: 0,
                to_table: 1,
                to_idx: 0,
            }],
        };

        let snapshot = SnapshotFile::with_rename_hints(Schema::default(), hints);
        let toml_str = snapshot.to_string();

        let parsed: SnapshotFile = toml_str.parse().unwrap();
        let stored = parsed.rename_hints.expect("rename_hints should be present");

        assert_eq!(stored.tables.len(), 1);
        assert_eq!(stored.tables[0].from, 0);
        assert_eq!(stored.tables[0].to, 1);

        assert_eq!(stored.columns.len(), 1);
        assert_eq!(stored.columns[0].from_table, 0);
        assert_eq!(stored.columns[0].from_col, 1);
        assert_eq!(stored.columns[0].to_table, 1);
        assert_eq!(stored.columns[0].to_col, 2);

        assert_eq!(stored.indices.len(), 1);
        assert_eq!(stored.indices[0].from_table, 0);
        assert_eq!(stored.indices[0].from_idx, 0);
        assert_eq!(stored.indices[0].to_table, 1);
        assert_eq!(stored.indices[0].to_idx, 0);
    }

    #[test]
    fn stored_hints_into_rename_hints_roundtrip() {
        let stored = StoredRenameHints {
            tables: vec![StoredTableRename { from: 2, to: 3 }],
            columns: vec![StoredColumnRename {
                from_table: 0,
                from_col: 5,
                to_table: 0,
                to_col: 6,
            }],
            indices: vec![StoredIndexRename {
                from_table: 1,
                from_idx: 2,
                to_table: 1,
                to_idx: 3,
            }],
        };

        let hints = stored.into_rename_hints();
        assert_eq!(hints.get_table(TableId(2)), Some(TableId(3)));
        assert_eq!(hints.get_table(TableId(0)), None);
        assert_eq!(
            hints.get_column(ColumnId {
                table: TableId(0),
                index: 5
            }),
            Some(ColumnId {
                table: TableId(0),
                index: 6
            })
        );
        assert_eq!(
            hints.get_index(IndexId {
                table: TableId(1),
                index: 2
            }),
            Some(IndexId {
                table: TableId(1),
                index: 3
            })
        );
    }
}
