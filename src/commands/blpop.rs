use super::types::ResponseError;
use crate::resp::types::{MultiBulk, RESPValue};
use crate::store::store::Store;
use futures::future::select_all;
use std::cell::RefCell;
use std::rc::Rc;
use tokio::sync::oneshot;

pub async fn blpop(
    input: &MultiBulk,
    store: &Rc<RefCell<Store>>,
) -> Result<RESPValue, ResponseError> {
    if input.len() < 3 {
        return Err(ResponseError::MalformedRequestError);
    }
    let raw_timeout: &RESPValue = input.last().ok_or(ResponseError::MalformedRequestError)?;
    let timeout = match raw_timeout {
        RESPValue::BulkString(s) => s
            .clone()
            .parse::<f64>()
            .map_err(|_| ResponseError::MalformedRequestError),
        _ => Err(ResponseError::MalformedRequestError),
    }?;

    let raw_popper_keys = &input[1..input.len() - 1];
    let popper_keys: Result<Vec<String>, ResponseError> = raw_popper_keys
        .iter()
        .map(|raw_key| match raw_key {
            RESPValue::BulkString(s) => Ok(s.clone()),
            _ => Err(ResponseError::MalformedRequestError),
        })
        .collect();
    let popper_keys = popper_keys?;

    let mut rxs: Vec<oneshot::Receiver<()>> = vec![];
    let id = store.borrow_mut().count();
    for p_key in &popper_keys {
        let elements = store.borrow_mut().lpop(p_key.as_str(), 1);
        match elements {
            Ok(elems) => {
                if elems.is_empty() {
                    rxs.push(store.borrow_mut().subscribe(id, &p_key));
                } else {
                    rxs.clear();
                    store.borrow_mut().unsubscribe(id, &popper_keys);
                    match elems.first() {
                        Some(e) => {
                            return Ok(RESPValue::Array(MultiBulk(vec![
                                RESPValue::BulkString(p_key.to_string()),
                                RESPValue::BulkString(e.clone()),
                            ])));
                        }
                        None => return Err(ResponseError::InternalError),
                    }
                }
            }
            Err(_) => {
                rxs.clear();
                store.borrow_mut().unsubscribe(id, &popper_keys);
                return Err(ResponseError::MalformedRequestError);
            }
        }
    }

    if timeout == 0.0 {
        let (_, idx, _) = select_all(rxs).await;
        let key = &popper_keys[idx];
        store.borrow_mut().unsubscribe(id, &popper_keys);
        let elem_res = store
            .borrow_mut()
            .lpop(key, 1)
            .map_err(|_| ResponseError::InternalError)?;
        match elem_res.first() {
            Some(e) => {
                return Ok(RESPValue::Array(MultiBulk(vec![
                    RESPValue::BulkString(key.to_string()),
                    RESPValue::BulkString(e.clone()),
                ])));
            }
            None => return Err(ResponseError::InternalError),
        }
    } else {
        tokio::select! {
            (_, idx, _) = select_all(rxs) => {
                let key = &popper_keys[idx];
                store.borrow_mut().unsubscribe(id, &popper_keys);
                let elem_res = store
                    .borrow_mut()
                    .lpop(key, 1)
                    .map_err(|_| ResponseError::InternalError)?;
                match elem_res.first() {
                    Some(e) => {
                        return Ok(RESPValue::Array(MultiBulk(vec![
                            RESPValue::BulkString(key.to_string()),
                            RESPValue::BulkString(e.clone()),
                        ])));
                    }
                    None => return Err(ResponseError::InternalError),
                }
            },
            _ = tokio::time::sleep(std::time::Duration::from_secs_f64(timeout)) => {
                store.borrow_mut().unsubscribe(id, &popper_keys);
                return Ok(RESPValue::NullArray)
            }
        }
    }
}
