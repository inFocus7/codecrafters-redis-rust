use super::types::ResponseError;
use crate::resp::types::RESPValue;

pub fn ping() -> Result<RESPValue, ResponseError> {
    Ok(RESPValue::SimpleString("PONG".to_string()))
}
