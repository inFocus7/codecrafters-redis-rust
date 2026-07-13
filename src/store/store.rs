use crate::store::types::StoreError;

use super::types::Entry;
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
        let mut entry = Entry::new(value);
        if let Some(exp) = expiry_ms {
            entry.with_expiry(exp);
        }
        self.data.insert(key, entry)
    }

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
}
