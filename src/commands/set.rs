use super::types::ResponseError;
use crate::resp::types::{MultiBulk, RESPValue};
use crate::store::store::Store;
use crate::store::types::Entry;
use std::cell::RefCell;
use std::rc::Rc;

pub fn set(input: &MultiBulk, store: &Rc<RefCell<Store>>) -> Result<RESPValue, ResponseError> {
    // Ok(RESPValue::SimpleString("PONG".to_string()))
    if input.len() != 3 {
        return Err(ResponseError::MalformedRequestError);
    }
    let raw_key = &input[1];
    let raw_value = &input[2];
    let key = match raw_key {
        RESPValue::BulkString(s) => s.to_string(),
        _ => return Err(ResponseError::MalformedRequestError),
    };
    let value = match raw_value {
        RESPValue::BulkString(s) => s.to_string(),
        _ => return Err(ResponseError::MalformedRequestError),
    };

    store.borrow_mut().set(key, Entry { value: value });
    Ok(RESPValue::SimpleString("OK".to_string()))
}
