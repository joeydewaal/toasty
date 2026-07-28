use super::Load;
use toasty_core::{Error, stmt};

/// Loader used by generated update builders when composed into a batch.
#[doc(hidden)]
pub struct UpdateCount;

impl Load for UpdateCount {
    type Output = u64;

    fn ty() -> stmt::Type {
        stmt::Type::U64
    }

    fn load(value: stmt::Value) -> Result<Self::Output, Error> {
        match value {
            stmt::Value::List(values) if values.is_empty() => Ok(0),
            stmt::Value::List(mut values) if values.len() == 1 => {
                <u64 as Load>::load(values.pop().unwrap())
            }
            value => <u64 as Load>::load(value),
        }
    }
}
