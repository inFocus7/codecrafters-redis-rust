use super::types::ResponseError;
use crate::resp::types::{MultiBulk, RESPValue};
use crate::store::store::Store;
use std::cell::RefCell;
use std::collections::VecDeque;
use std::rc::Rc;

pub fn lpush(input: &MultiBulk, store: &Rc<RefCell<Store>>) -> Result<RESPValue, ResponseError> {
    if input.len() < 3 {
        return Err(ResponseError::MalformedRequestError);
    }
    let raw_key = &input[1];
    let key = match raw_key {
        RESPValue::BulkString(s) => s.clone(),
        _ => return Err(ResponseError::MalformedRequestError),
    };

    let mut elements = VecDeque::<String>::new();
    for raw_val in input[2..].iter() {
        let val = match raw_val {
            RESPValue::BulkString(s) => s.clone(),
            _ => return Err(ResponseError::MalformedRequestError),
        };
        elements.push_back(val);
    }

    // TODO: I need to figure out how to properly have generic errors so i can propagate the StoreError upwards...
    let n = store
        .borrow_mut()
        .lpush(key, elements)
        .map_err(|_| ResponseError::InternalError)?;

    Ok(RESPValue::Integer(n as i64))
}
