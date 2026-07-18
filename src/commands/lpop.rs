use super::types::ResponseError;
use crate::resp::types::{MultiBulk, RESPValue};
use crate::store::store::Store;
use std::cell::RefCell;
use std::collections::VecDeque;
use std::rc::Rc;

pub fn lpop(input: &MultiBulk, store: &Rc<RefCell<Store>>) -> Result<RESPValue, ResponseError> {
    if input.len() < 2 || input.len() > 3 {
        return Err(ResponseError::MalformedRequestError);
    }
    let raw_key = &input[1];
    let key = match raw_key {
        RESPValue::BulkString(s) => s.clone(),
        _ => return Err(ResponseError::MalformedRequestError),
    };

    let mut num_elements: usize = 1;
    if input.len() == 3 {
        let raw_num = &input[2];
        num_elements = match raw_num {
            RESPValue::BulkString(rs) => usize::from_str_radix(rs.as_str(), 10)
                .map_err(|_| ResponseError::MalformedRequestError)?,
            _ => return Err(ResponseError::MalformedRequestError),
        }
    }

    let elems = store
        .borrow_mut()
        .lpop(&key, num_elements)
        .map_err(|_| ResponseError::InternalError)?;

    if elems.is_empty() {
        Ok(RESPValue::NullBulkString)
    } else if elems.len() == 1 {
        Ok(RESPValue::BulkString(elems[0].clone()))
    } else {
        Ok(RESPValue::Array(MultiBulk::from(elems)))
    }
}
