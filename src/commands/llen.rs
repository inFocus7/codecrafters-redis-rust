use super::types::ResponseError;
use crate::resp::types::{MultiBulk, RESPValue};
use crate::store::store::Store;
use std::cell::RefCell;
use std::rc::Rc;

pub fn llen(input: &MultiBulk, store: &Rc<RefCell<Store>>) -> Result<RESPValue, ResponseError> {
    if input.len() != 2 {
        return Err(ResponseError::MalformedRequestError);
    }
    let raw_key = &input[1];
    let key = match raw_key {
        RESPValue::BulkString(s) => s.to_string(),
        _ => return Err(ResponseError::MalformedRequestError),
    };

    let len = store
        .borrow_mut()
        .llen(&key)
        .map_err(|_| ResponseError::InternalError)?;

    return Ok(RESPValue::Integer(
        i64::try_from(len).map_err(|_| ResponseError::InternalError)?,
    ));
}
