mod create_table;
mod delete_by_key;
mod find_pk_by_index;
mod get_by_key;
mod insert;
mod query_pk;
mod scan;
mod update_by_key;
mod upsert;

use super::{
    AttributeDefinition, AttributeValue, BillingMode, Connection, Delete, ExprAttrs,
    GlobalSecondaryIndex, KeysAndAttributes, Projection, ProjectionType, Put, PutRequest,
    ReturnValuesOnConditionCheckFailure, SdkError, TransactWriteItem, TransactWriteItemsError,
    TypeExt, Update, UpdateItemError, Value, WriteRequest, ddb_expression, ddb_key, ddb_key_schema,
    deserialize_ddb_cursor, item_to_record, serialize_ddb_cursor,
};
use std::collections::HashMap;
use toasty_core::{
    Result, Schema,
    driver::operation,
    schema::db::{self, Table},
    stmt,
};

struct RecordInput<'a>(&'a stmt::ValueRecord);

impl stmt::Input for RecordInput<'_> {
    fn resolve_ref(
        &mut self,
        expr_reference: &stmt::ExprReference,
        projection: &stmt::Projection,
    ) -> Option<stmt::Expr> {
        match expr_reference {
            stmt::ExprReference::Column(col) => {
                Some(self.0.fields[col.column].entry(projection).to_expr())
            }
            _ => None,
        }
    }
}

fn filter_failed(
    old_item: Option<&HashMap<String, AttributeValue>>,
    table: &db::Table,
    filter: Option<&stmt::Expr>,
) -> Result<bool> {
    let Some(filter) = filter else {
        return Ok(false);
    };

    let Some(item) = old_item else {
        return Ok(true);
    };

    let record = item_to_record(item, table.columns.iter())?;
    let matched = filter.eval_bool(RecordInput(&record))?;
    Ok(!matched)
}
