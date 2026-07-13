use super::types::Entry;
use ahash::RandomState;
use std::collections::HashMap;

pub struct Store {
    data: HashMap<String, Entry, RandomState>,
}

impl Store {
    pub fn new() -> Self {
        Store {
            data: HashMap::with_hasher(RandomState::new()),
        }
    }

    pub fn set(&mut self, key: String, value: Entry) -> Option<Entry> {
        self.data.insert(key, value)
    }

    pub fn get(&self, key: &str) -> Option<&Entry> {
        self.data.get(key)
    }

    pub fn delete(&mut self, key: &str) -> Option<Entry> {
        self.data.remove(key)
    }
}
