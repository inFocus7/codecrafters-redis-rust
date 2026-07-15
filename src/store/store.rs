use crate::store::types::StoreError;

use super::types::{Entry, Value};
use ahash::RandomState;
use std::collections::HashMap;

// TODO: active (configurable) expiration

pub struct Store {
    data: HashMap<String, Entry, RandomState>,
}

impl Store {
    pub fn new() -> Self {
        Store {
            data: HashMap::with_hasher(RandomState::new()),
        }
    }

    pub fn set(&mut self, key: String, value: String, expiry_ms: Option<u64>) -> Option<Entry> {
        let mut entry = Entry::new(Value::String(value));
        if let Some(exp) = expiry_ms {
            entry.with_expiry(exp);
        }
        self.data.insert(key, entry)
    }

    // TODO: Right now we require mutable store, meaning borrow_mut(), but i can instead delete-if-expired after the get() to keep this immutable.
    // Something like: Ok(val) => exists, ok; Ok(None) => exists, expired {then caller does a .delete() on their end}; None | Err(NotExists) => DNE; Err(val) => error
    pub fn get(&mut self, key: &str) -> Result<Option<&Entry>, StoreError> {
        let expired = if let Some(entry) = self.data.get(key) {
            if let Some(exp) = entry.expiry {
                let now = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map_err(|_| StoreError::InternalError)?;
                now.as_millis() as u64 > exp
            } else {
                false
            }
        } else {
            false
        };

        if expired {
            self.delete(key);
            return Ok(None);
        }
        return Ok(self.data.get(key));
    }

    pub fn delete(&mut self, key: &str) -> Option<Entry> {
        self.data.remove(key)
    }

    pub fn rpush(&mut self, key: String, elements: Vec<String>) -> Result<usize, StoreError> {
        match self.data.get_mut(&key) {
            Some(v) => match &mut v.value {
                Value::String(_) => return Err(StoreError::KeyTaken),
                Value::List(l) => {
                    l.extend(elements);
                    Ok(l.len())
                }
            },
            None => {
                let len = elements.len();
                self.data.insert(key, Entry::new(Value::List(elements)));
                Ok(len)
            }
        }
    }
}
