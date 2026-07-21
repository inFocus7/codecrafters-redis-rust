use crate::store::types::StoreError;

use super::types::{Entry, Value};
use ahash::RandomState;
use std::cmp::min;
use std::collections::BTreeMap;
use std::collections::HashMap;
use std::collections::VecDeque;
use tokio::sync::oneshot;

// TODO: Active (configurable) expiration, instead of lazy.
// IDEA: Every S seconds get N random keys and delete if expired.

pub struct Store {
    // the kv data store => key: entry
    data: HashMap<String, Entry, RandomState>,
    // Map from blocked key to BTreeMap (ordered tree based on key)
    // We can use BTreeMap instead of needing a IndexMap for ordering, since we'll be using a counter for keys
    waiters: HashMap<String, BTreeMap<u64, oneshot::Sender<()>>, RandomState>,
    counter: u64,
}

impl Store {
    pub fn new() -> Self {
        Store {
            data: HashMap::with_hasher(RandomState::new()),
            waiters: HashMap::with_hasher(RandomState::new()),
            counter: 0,
        }
    }

    // updates counter, should call once as-needed per batch of work
    pub fn count(&mut self) -> u64 {
        self.counter += 1;
        return self.counter;
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
        if self.handle_expire(key)? {
            return Ok(None);
        }

        Ok(self.data.get(key))
    }

    pub fn delete(&mut self, key: &str) -> Option<Entry> {
        self.data.remove(key)
    }

    pub fn rpush(&mut self, key: String, elements: VecDeque<String>) -> Result<usize, StoreError> {
        let res = match self.data.get_mut(&key) {
            Some(v) => match &mut v.value {
                Value::String(_) => Err(StoreError::KeyTaken)?,
                Value::List(l) => {
                    l.extend(elements);
                    l.len()
                }
            },
            None => {
                let len = elements.len();
                self.data
                    .insert(key.clone(), Entry::new(Value::List(elements)));
                len
            }
        };

        if let Some(m) = self.waiters.get_mut(&key)
            && !m.is_empty()
        {
            // let first non-dropped rx know it's ready
            while let Some((_, tx)) = m.pop_first() {
                if tx.send(()).is_ok() {
                    break;
                }
            }
        }

        Ok(res)
    }

    pub fn lpush(&mut self, key: String, elements: VecDeque<String>) -> Result<usize, StoreError> {
        let k = key.clone();
        let res = match self.data.get_mut(&key) {
            Some(v) => match &mut v.value {
                Value::String(_) => Err(StoreError::KeyTaken)?,
                Value::List(l) => {
                    elements.into_iter().for_each(|e| l.push_front(e));
                    l.len()
                }
            },
            None => {
                // could also make elements contiguous and reverse inline
                let mut l = VecDeque::new();
                for e in elements {
                    l.push_front(e);
                }
                let len = l.len();
                self.data.insert(key, Entry::new(Value::List(l)));
                len
            }
        };

        if let Some(m) = self.waiters.get_mut(&k)
            && !m.is_empty()
        {
            // let first non-dropped rx know it's ready
            while let Some((_, tx)) = m.pop_first() {
                if tx.send(()).is_ok() {
                    break;
                }
            }
        }

        return Ok(res);
    }

    pub fn lrange(
        &mut self,
        key: &str,
        start: isize,
        stop: isize,
    ) -> Result<Vec<String>, StoreError> {
        if self.handle_expire(key)? {
            return Ok(vec![]);
        }

        if let Some(entry) = self.data.get(key) {
            match &entry.value {
                Value::List(l) => {
                    if start > 0 && start as usize > l.len() {
                        return Ok(vec![]);
                    }
                    let norm_stop = normalize_idx(stop, l.len());
                    let norm_start = normalize_idx(start, l.len());

                    if norm_start > norm_stop {
                        return Ok(vec![]);
                    }

                    let mut res =
                        Vec::<String>::with_capacity((norm_stop - norm_start) as usize + 1);

                    for i in 0..(norm_stop - norm_start + 1) {
                        res.push(l[norm_start + i].clone());
                    }

                    return Ok(res);
                }
                Value::String(_) => return Err(StoreError::WrongType),
            }
        } else {
            Ok(vec![])
        }
    }

    pub fn llen(&mut self, key: &str) -> Result<usize, StoreError> {
        if self.handle_expire(key)? {
            return Ok(0);
        }

        if let Some(entry) = self.data.get(key) {
            match &entry.value {
                Value::List(l) => Ok(l.len()),
                _ => Err(StoreError::WrongType),
            }
        } else {
            Ok(0)
        }
    }

    pub fn lpop(&mut self, key: &str, num_elements: usize) -> Result<Vec<String>, StoreError> {
        if self.handle_expire(key)? {
            return Ok(vec![]);
        }

        // if len too big, just pop all, keep array
        // if key DNE: nil reply
        if let Some(entry) = self.data.get_mut(key) {
            match &mut entry.value {
                Value::List(l) => {
                    if num_elements >= l.len() {
                        let items = l.clone();
                        l.clear();
                        return Ok(Vec::<String>::from(items));
                    }

                    let mut items = Vec::<String>::with_capacity(num_elements);
                    for _ in 0..num_elements {
                        if let Some(pop_front) = l.pop_front() {
                            items.push(pop_front);
                        };
                    }
                    return Ok(items);
                }
                _ => Err(StoreError::WrongType),
            }
        } else {
            Ok(vec![])
        }
    }

    // subscribes to a key, should call counter() to get latest count
    // not doing counter within the method so requests can keep their count as id (but may change)
    pub fn subscribe(&mut self, id: u64, key: &str) -> oneshot::Receiver<()> {
        let (tx, rx) = oneshot::channel::<()>();
        self.waiters
            .entry(key.to_string())
            .or_insert_with(BTreeMap::<u64, oneshot::Sender<()>>::new)
            .insert(id, tx);
        rx
    }

    pub fn unsubscribe(&mut self, id: u64, keys: &Vec<String>) {
        for key in keys {
            if let Some(m) = self.waiters.get_mut(key) {
                m.remove(&id);
            }
        }
    }

    fn handle_expire(&mut self, key: &str) -> Result<bool, StoreError> {
        if let Some(entry) = self.data.get(key)
            && entry.is_expired()?
        {
            self.delete(key);
            return Ok(true);
        }
        Ok(false)
    }
}

fn normalize_idx(idx: isize, len: usize) -> usize {
    if idx < 0 {
        let abs = idx.unsigned_abs();
        // not using euclidian remainder since we need to clamp if negative is too negative
        if abs > len {
            0 // clamp
        } else {
            len - abs
        }
    } else {
        min(idx as usize, len - 1) // clamp to last
    }
}
