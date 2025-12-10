/// Error returned when a command is rejected or fails.
#[derive(Debug, Clone)]
pub struct CommandError {
    /// The error classification
    pub code: ErrorCode,
    /// Human-readable error message
    pub message: String,
}

/// Classification of command errors.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorCode {
    /// Business rule violation - the command was understood but rejected.
    /// Example: "Insufficient funds"
    Rejected,

    /// The input was malformed or invalid.
    /// Example: "Amount must be positive"
    InvalidInput,

    /// An unexpected error occurred in the handler.
    /// Example: Deserialization failure, logic bug
    Internal,
}

impl CommandError {
    /// Create a rejection error for business rule violations.
    pub fn rejected(message: impl Into<String>) -> Self {
        Self {
            code: ErrorCode::Rejected,
            message: message.into(),
        }
    }

    /// Create an invalid input error.
    pub fn invalid_input(message: impl Into<String>) -> Self {
        Self {
            code: ErrorCode::InvalidInput,
            message: message.into(),
        }
    }

    /// Create an internal error.
    pub fn internal(message: impl Into<String>) -> Self {
        Self {
            code: ErrorCode::Internal,
            message: message.into(),
        }
    }
}

impl std::fmt::Display for CommandError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}: {}", self.code, self.message)
    }
}

impl std::error::Error for CommandError {}

/// Error during event serialization/deserialization.
#[derive(Debug)]
pub struct SerializationError {
    pub message: String,
}

impl SerializationError {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl std::fmt::Display for SerializationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "serialization error: {}", self.message)
    }
}

impl std::error::Error for SerializationError {}

impl From<serde_json::Error> for SerializationError {
    fn from(err: serde_json::Error) -> Self {
        Self::new(err.to_string())
    }
}
