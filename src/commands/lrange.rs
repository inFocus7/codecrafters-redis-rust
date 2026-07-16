use super::types::ResponseError;
use crate::resp::types::{MultiBulk, RESPValue};
use crate::store::store::Store;
use std::cell::RefCell;
use std::rc::Rc;

pub fn lrange(input: &MultiBulk, store: &Rc<RefCell<Store>>) -> Result<RESPValue, ResponseError> {
    if input.len() != 4 {
        return Err(ResponseError::MalformedRequestError);
    }
    let raw_key = &input[1];
    let key = match raw_key {
        RESPValue::BulkString(s) => s,
        _ => return Err(ResponseError::MalformedRequestError),
    };

    let raw_start = &input[2];
    let start = match raw_start {
        RESPValue::BulkString(s) => {
            isize::from_str_radix(s, 10).map_err(|_| ResponseError::InternalError)?
        }
        _ => return Err(ResponseError::MalformedRequestError),
    };

    let raw_stop = &input[3];
    let stop = match raw_stop {
        RESPValue::BulkString(s) => {
            isize::from_str_radix(s, 10).map_err(|_| ResponseError::InternalError)?
        }
        _ => return Err(ResponseError::MalformedRequestError),
    };

    let res_raw = store
        .borrow_mut()
        .lrange(key, start, stop)
        .map_err(|_| ResponseError::InternalError)?;

    // convert raw String vector into array of BulkStrings
    Ok(RESPValue::Array(MultiBulk::from_iter(
        res_raw.into_iter().map(|s| RESPValue::BulkString(s)),
    )))
}
