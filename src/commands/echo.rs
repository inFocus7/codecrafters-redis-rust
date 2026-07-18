use super::types::ResponseError;
use crate::resp::types::{MultiBulk, RESPValue};

pub fn echo(input: &MultiBulk) -> Result<RESPValue, ResponseError> {
    if input.is_empty() {
        return Err(ResponseError::MalformedRequestError);
    }

    if input.len() == 1 {
        // empty response
        return Ok(RESPValue::BulkString("".to_string()));
    }

    // raw content to echo back
    match &input[1] {
        RESPValue::BulkString(s) => return Ok(RESPValue::BulkString(s.clone())),
        _ => return Err(ResponseError::MalformedRequestError),
    }
}
