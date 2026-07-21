use crate::{
    Result,
    engine::{
        eval,
        exec::{Action, Exec, Output, VarId},
    },
};
use toasty_core::{
    driver::{ExecResponse, Rows},
    stmt::ValueStream,
};

/// Narrows update-returning rows while execution is still transactional.
#[derive(Debug)]
pub(crate) struct ReturnFirst {
    pub(crate) input: VarId,
    pub(crate) selector: Option<VarId>,
    pub(crate) output: Output,
    pub(crate) key: Option<eval::Func>,
    pub(crate) required: bool,
}

impl Exec<'_> {
    pub(super) async fn action_return_first(&mut self, action: &ReturnFirst) -> Result<()> {
        let response = self.vars.load(action.input).await?;
        let rows = response.values.into_value_stream().collect().await?;
        let row = if let Some(selector) = action.selector {
            let selected = self
                .vars
                .load(selector)
                .await?
                .values
                .into_value_stream()
                .collect()
                .await?
                .into_iter()
                .next();
            let key = action.key.as_ref().unwrap();

            match selected {
                Some(selected) => rows
                    .into_iter()
                    .find_map(|row| {
                        let row_key = key.eval(&self.engine.schema, std::slice::from_ref(&row));
                        match row_key {
                            Ok(row_key) if row_key == selected => Some(Ok(row)),
                            Ok(_) => None,
                            Err(error) => Some(Err(error)),
                        }
                    })
                    .transpose()?,
                None => None,
            }
        } else {
            rows.into_iter().next()
        };
        let rows = row.into_iter().collect::<Vec<_>>();

        if action.required && rows.is_empty() {
            return Err(toasty_core::Error::record_not_found(
                "update returned no matching records",
            ));
        }

        self.vars.store(
            action.output.var,
            action.output.num_uses,
            ExecResponse {
                values: Rows::Stream(ValueStream::from_vec(rows)),
                next_cursor: response.next_cursor,
                prev_cursor: response.prev_cursor,
            },
        );

        Ok(())
    }
}

impl From<ReturnFirst> for Action {
    fn from(value: ReturnFirst) -> Self {
        Self::ReturnFirst(value)
    }
}
