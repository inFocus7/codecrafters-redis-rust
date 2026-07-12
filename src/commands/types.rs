#[derive(Debug, PartialEq, Eq)]
pub enum ResponseError {
    // BuildError,              // error building response
    MalformedRequestError,   // invalid input request
    UnsupportedCommandError, // unsupported commands (generic. either missing or unknown)
}

impl std::error::Error for ResponseError {}
impl std::fmt::Display for ResponseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            // ResponseError::BuildError => {
            //     write!(f, "error building response")
            // }
            ResponseError::MalformedRequestError => {
                write!(f, "malformed request")
            }
            ResponseError::UnsupportedCommandError => {
                write!(f, "unsupported command")
            }
        }
    }
}
