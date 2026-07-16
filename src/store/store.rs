use crate::store::types::StoreError;

use super::types::{Entry, Value};
use ahash::RandomState;
use std::cmp::min;
use std::collections::HashMap;
use std::ops::Index;

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
        if let Some(entry) = self.data.get(key)
            && entry.is_expired()?
        {
            self.delete(key);
            return Ok(None);
        }
        Ok(self.data.get(key))
    }

    fn has(&mut self, key: &str) -> bool {
        self.data.contains_key(key)
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

    pub fn lrange(
        &mut self,
        key: &str,
        start: isize,
        stop: isize,
    ) -> Result<Vec<String>, StoreError> {
        if let Some(entry) = self.data.get(key)
            && entry.is_expired()?
        {
            self.delete(key);
            return Ok(vec![]);
        }

        if let Some(entry) = self.data.get(key) {
            match &entry.value {
                Value::List(l) => {
                    let stop_c = min(stop, l.len() as isize - 1);

                    // normalize negatives from end of list
                    let norm_start = start.rem_euclid(l.len() as isize) as usize;
                    let norm_stop = stop_c.rem_euclid(l.len() as isize) as usize;

                    if norm_start > l.len() || norm_start > norm_stop {
                        return Ok(vec![]);
                    }

                    let mut res =
                        Vec::<String>::with_capacity((norm_stop - norm_start) as usize + 1);

                    for i in 0..(norm_stop - norm_start + 1) {
                        res.push(l[norm_start + i].to_string());
                    }

                    return Ok(res);
                }
                Value::String(_) => return Err(StoreError::WrongType),
            }
        } else {
            Ok(vec![])
        }
    }
}
