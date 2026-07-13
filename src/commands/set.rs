use super::types::ResponseError;
use crate::resp::types::{MultiBulk, RESPValue};
use crate::store::store::Store;
use std::cell::RefCell;
use std::rc::Rc;
use std::u64;
use twox_hash::xxhash3_64;

enum Expiry {
    Ex(u64),   // expiration time in seconds
    Px(u64),   // expiration time in milliseconds
    Exat(u64), // exact unix time expiration in seconds
    Pxat(u64), // exact unit time expiration in milliseconds
    KeepTtl,   // retain TTL; meaning read existing and keep val
}

enum Condition {
    Nx,            // only set if key does not exist
    Xx,            // only set if key already exists
    Ifeq(String),  // only set if value == this
    Ifne(String),  // only set if value != this
    Ifdeq(String), // only set if hash digest of current value == this; 64-bit, like redis
    Ifdne(String), // only set if hash digest of current value != this; 64-bit, like redis
}

struct SetOptions {
    expiry: Option<Expiry>,
    condition: Option<Condition>,
}

impl SetOptions {
    fn new() -> Self {
        SetOptions {
            expiry: None,
            condition: None,
        }
    }
}

pub fn set(input: &MultiBulk, store: &Rc<RefCell<Store>>) -> Result<RESPValue, ResponseError> {
    if input.len() < 3 {
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

    // anything after SET key value are optional arguments
    let opts = parse_opts(input)?;

    let mut expiry_ms: Option<u64> = None;
    match opts.expiry {
        Some(Expiry::Ex(secs)) => {
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_err(|_| ResponseError::InternalError)?;
            let ms = secs * 1000;
            expiry_ms = Some(now.as_millis() as u64 + ms);
        }
        Some(Expiry::Px(ms)) => {
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_err(|_| ResponseError::InternalError)?;
            expiry_ms = Some(now.as_millis() as u64 + ms);
        }
        Some(Expiry::Exat(secs)) => expiry_ms = Some(secs * 1000),
        Some(Expiry::Pxat(ms)) => expiry_ms = Some(ms),
        Some(Expiry::KeepTtl) => {
            if let Some(v) = store
                .borrow_mut()
                .get(&key)
                .map_err(|_| ResponseError::InternalError)?
            {
                expiry_ms = v.expiry
            }
        }
        None => {}
    }
    match opts.condition {
        Some(Condition::Nx) => {
            let should_set = store
                .borrow_mut()
                .get(&key)
                .map_err(|_| ResponseError::InternalError)?
                .is_none();
            if should_set {
                store.borrow_mut().set(key, value, expiry_ms);
            }
        }
        Some(Condition::Xx) => {
            let should_set = store
                .borrow_mut()
                .get(&key)
                .map_err(|_| ResponseError::InternalError)?
                .is_some();
            if should_set {
                store.borrow_mut().set(key, value, expiry_ms);
            }
        }
        Some(Condition::Ifeq(s)) => {
            let should_set = store
                .borrow_mut()
                .get(&key)
                .map_err(|_| ResponseError::InternalError)?
                .map(|v| v.value == s) // if Some && equal
                .unwrap_or(false); // if None
            if should_set {
                store.borrow_mut().set(key, value, expiry_ms);
            }
        }
        Some(Condition::Ifne(s)) => {
            let should_set = store
                .borrow_mut()
                .get(&key)
                .map_err(|_| ResponseError::InternalError)?
                .map(|v| v.value != s) // if Some && not equal
                .unwrap_or(true); // if None
            if should_set {
                store.borrow_mut().set(key, value, expiry_ms);
            }
        }
        Some(Condition::Ifdeq(s)) => {
            let s_int =
                u64::from_str_radix(&s, 16).map_err(|_| ResponseError::MalformedRequestError)?; // convert hex to u128 int
            let should_set = store
                .borrow_mut()
                .get(&key)
                .map_err(|_| ResponseError::InternalError)?
                .map(|v| xxhash3_64::Hasher::oneshot(v.value.as_bytes()) == s_int) // if the hashes match
                .unwrap_or(false); // if None, don't create
            if should_set {
                store.borrow_mut().set(key, value, expiry_ms);
            }
        }
        Some(Condition::Ifdne(s)) => {
            let s_int =
                u64::from_str_radix(&s, 16).map_err(|_| ResponseError::MalformedRequestError)?; // convert hex to u128 int
            let should_set = store
                .borrow_mut()
                .get(&key)
                .map_err(|_| ResponseError::InternalError)?
                .map(|v| xxhash3_64::Hasher::oneshot(v.value.as_bytes()) == s_int) // if the hashes match
                .unwrap_or(true); // if None, create
            if should_set {
                store.borrow_mut().set(key, value, expiry_ms);
            }
        }
        None => {
            store.borrow_mut().set(key, value, expiry_ms);
        }
    }

    // TODO: actually process response? codecrafter section just states return OK
    Ok(RESPValue::SimpleString("OK".to_string()))
}

fn parse_opts(input: &MultiBulk) -> Result<SetOptions, ResponseError> {
    let mut opts = SetOptions::new();
    let mut i = 3; // start after the command, key, and value

    while i < input.len() {
        let raw_opt = &input[i];
        let opt = match raw_opt {
            RESPValue::BulkString(s) => s.to_lowercase(),
            _ => return Err(ResponseError::MalformedRequestError),
        };

        match opt.as_str() {
            "ex" => {
                if opts.expiry.is_some() {
                    return Err(ResponseError::MalformedRequestError);
                }
                if i + 1 >= input.len() {
                    return Err(ResponseError::MalformedRequestError);
                }

                let raw_val = &input[i + 1];
                let val = match raw_val {
                    RESPValue::BulkString(s) => s.to_string(),
                    _ => return Err(ResponseError::MalformedRequestError),
                };
                let ttl_sec = val
                    .parse::<u64>()
                    .map_err(|_| ResponseError::MalformedRequestError)?;
                opts.expiry = Some(Expiry::Ex(ttl_sec));

                i += 2;
            }
            "px" => {
                if opts.expiry.is_some() {
                    return Err(ResponseError::MalformedRequestError);
                }
                if i + 1 >= input.len() {
                    return Err(ResponseError::MalformedRequestError);
                }

                let raw_val = &input[i + 1];
                let val = match raw_val {
                    RESPValue::BulkString(s) => s.to_string(),
                    _ => return Err(ResponseError::MalformedRequestError),
                };

                let ttl_ms = val
                    .parse::<u64>()
                    .map_err(|_| ResponseError::MalformedRequestError)?;
                opts.expiry = Some(Expiry::Px(ttl_ms));

                i += 2;
            }
            "exat" => {
                if opts.expiry.is_some() {
                    return Err(ResponseError::MalformedRequestError);
                }
                if i + 1 >= input.len() {
                    return Err(ResponseError::MalformedRequestError);
                }

                let raw_val = &input[i + 1];
                let val = match raw_val {
                    RESPValue::BulkString(s) => s.to_string(),
                    _ => return Err(ResponseError::MalformedRequestError),
                };

                let expiry_sec = val
                    .parse::<u64>()
                    .map_err(|_| ResponseError::MalformedRequestError)?;
                opts.expiry = Some(Expiry::Exat(expiry_sec));

                i += 2;
            }
            "pxat" => {
                if opts.expiry.is_some() {
                    return Err(ResponseError::MalformedRequestError);
                }
                if i + 1 >= input.len() {
                    return Err(ResponseError::MalformedRequestError);
                }

                let raw_val = &input[i + 1];
                let val = match raw_val {
                    RESPValue::BulkString(s) => s.to_string(),
                    _ => return Err(ResponseError::MalformedRequestError),
                };

                let expiry_ms = val
                    .parse::<u64>()
                    .map_err(|_| ResponseError::MalformedRequestError)?;
                opts.expiry = Some(Expiry::Pxat(expiry_ms));

                i += 2;
            }
            "keepttl" => {
                if opts.expiry.is_some() {
                    return Err(ResponseError::MalformedRequestError);
                }

                opts.expiry = Some(Expiry::KeepTtl);
                i += 1;
            }
            "nx" => {
                if opts.condition.is_some() {
                    return Err(ResponseError::MalformedRequestError);
                }

                opts.condition = Some(Condition::Nx);
                i += 1;
            }
            "xx" => {
                if opts.condition.is_some() {
                    return Err(ResponseError::MalformedRequestError);
                }

                opts.condition = Some(Condition::Xx);
                i += 1;
            }
            "ifeq" => {
                if opts.condition.is_some() {
                    return Err(ResponseError::MalformedRequestError);
                }
                if i + 1 >= input.len() {
                    return Err(ResponseError::MalformedRequestError);
                }

                let raw_val = &input[i + 1];
                let val = match raw_val {
                    RESPValue::BulkString(s) => s.to_string(),
                    _ => return Err(ResponseError::MalformedRequestError),
                };

                opts.condition = Some(Condition::Ifeq(val));
                i += 2;
            }
            "ifne" => {
                if opts.condition.is_some() {
                    return Err(ResponseError::MalformedRequestError);
                }
                if i + 1 >= input.len() {
                    return Err(ResponseError::MalformedRequestError);
                }

                let raw_val = &input[i + 1];
                let val = match raw_val {
                    RESPValue::BulkString(s) => s.to_string(),
                    _ => return Err(ResponseError::MalformedRequestError),
                };

                opts.condition = Some(Condition::Ifne(val));
                i += 2;
            }
            "ifdeq" => {
                if opts.condition.is_some() {
                    return Err(ResponseError::MalformedRequestError);
                }
                if i + 1 >= input.len() {
                    return Err(ResponseError::MalformedRequestError);
                }

                let raw_val = &input[i + 1];
                let val = match raw_val {
                    RESPValue::BulkString(s) => s.to_string(),
                    _ => return Err(ResponseError::MalformedRequestError),
                };

                opts.condition = Some(Condition::Ifdeq(val));
                i += 2;
            }
            "ifdne" => {
                if opts.condition.is_some() {
                    return Err(ResponseError::MalformedRequestError);
                }
                if i + 1 >= input.len() {
                    return Err(ResponseError::MalformedRequestError);
                }

                let raw_val = &input[i + 1];
                let val = match raw_val {
                    RESPValue::BulkString(s) => s.to_string(),
                    _ => return Err(ResponseError::MalformedRequestError),
                };

                opts.condition = Some(Condition::Ifdne(val));
                i += 2;
            }
            _ => return Err(ResponseError::MalformedRequestError),
        }
    }
    Ok(opts)
}
