#[derive(Debug, PartialEq, Eq)]
pub enum StoreError {
    InternalError, // internal error
}

impl std::error::Error for StoreError {}
impl std::fmt::Display for StoreError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            StoreError::InternalError => {
                write!(f, "internal error")
            }
        }
    }
}

pub struct Entry {
    pub value: String,
    pub expiry: Option<u64>, // Unix timestamp in milliseconds when the entry expires. None means no expiry.
}

impl Entry {
    pub fn new(value: String) -> Self {
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
