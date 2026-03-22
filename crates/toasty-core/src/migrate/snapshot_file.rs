use std::fmt;
use std::path::Path;
use std::str::FromStr;

use serde::{Deserialize, Serialize};
use toml_edit::{DocumentMut, Item};

use crate::schema::db::Schema;
use crate::{Error, Result};

const SNAPSHOT_FILE_VERSION: u32 = 1;

/// Snapshot file containing the current database schema state
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnapshotFile {
    /// Snapshot file format version
    version: u32,

    /// The database schema
    pub schema: Schema,
}

impl SnapshotFile {
    /// Create a new snapshot file with the given schema
    pub fn new(schema: Schema) -> Self {
        Self {
            version: SNAPSHOT_FILE_VERSION,
            schema,
        }
    }

    /// Load a snapshot file from a TOML file
    pub fn load(path: impl AsRef<Path>) -> Result<Self> {
        let contents = std::fs::read_to_string(path.as_ref())
            .map_err(|e| Error::migration_failed(e.to_string()))?;
        contents.parse()
    }

    /// Save the snapshot file to a TOML file
    pub fn save(&self, path: impl AsRef<Path>) -> Result<()> {
        std::fs::write(path.as_ref(), self.to_string())
            .map_err(|e| Error::migration_failed(e.to_string()))?;
        Ok(())
    }

    fn to_toml_document(&self) -> Result<DocumentMut> {
        let mut doc = toml_edit::ser::to_document(self)
            .map_err(|e| Error::migration_failed(e.to_string()))?;
        for (_key, item) in doc.as_table_mut().iter_mut() {
            if item.is_inline_table() {
                let mut placeholder = Item::None;
                std::mem::swap(item, &mut placeholder);
                let mut table = placeholder.into_table().unwrap();

                for (_key, item) in table.iter_mut() {
                    if item.is_array() {
                        let mut placeholder = Item::None;
                        std::mem::swap(item, &mut placeholder);
                        let mut array = placeholder.into_array_of_tables().unwrap();

                        for table in array.iter_mut() {
                            for (_key, item) in table.iter_mut() {
                                if item.is_array() {
                                    let mut placeholder = Item::None;
                                    std::mem::swap(item, &mut placeholder);
                                    let array = placeholder.into_array_of_tables().unwrap();
                                    *item = array.into();
                                }
                            }
                        }

                        *item = array.into();
                    }
                }

                *item = table.into();
            }
        }

        Ok(doc)
    }
}

impl FromStr for SnapshotFile {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self> {
        let file: SnapshotFile =
            toml::from_str(s).map_err(|e| Error::migration_failed(e.to_string()))?;

        if file.version != SNAPSHOT_FILE_VERSION {
            return Err(Error::migration_failed(format!(
                "Unsupported snapshot file version: {}. Expected version {}",
                file.version, SNAPSHOT_FILE_VERSION
            )));
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
