#[derive(Debug, PartialEq, Eq)]
pub enum StoreError {
    InternalError, // internal error
    KeyTaken,
}

impl std::error::Error for StoreError {}
impl std::fmt::Display for StoreError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            StoreError::InternalError => {
                write!(f, "internal error")
            }
            StoreError::KeyTaken => {
                write!(f, "key already taken")
            }
        }
    }
}

pub enum Value {
    String(String),
    List(Vec<String>),
}

pub struct Entry {
    pub value: Value,
    pub expiry: Option<u64>, // Unix timestamp in milliseconds when the entry expires. None means no expiry.
}

impl Entry {
    pub fn new(value: Value) -> Self {
        Entry {
            value,
            expiry: None,
        }
    }

    pub fn with_expiry(&mut self, expiry: u64) -> &mut Self {
        self.expiry = Some(expiry);
        self
    }
}
