#[derive(Debug, PartialEq, Eq)]
pub enum ResponseError {
    MalformedRequestError,   // invalid input request
    UnsupportedCommandError, // unsupported commands (generic. either missing or unknown)
    InternalError,           // internal error
}

impl std::error::Error for ResponseError {}
impl std::fmt::Display for ResponseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ResponseError::MalformedRequestError => {
                write!(f, "malformed request")
            }
            ResponseError::UnsupportedCommandError => {
                write!(f, "unsupported command")
            }
            ResponseError::InternalError => {
                write!(f, "internal error")
            }
        }
    }
}
