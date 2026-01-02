use std::fmt;

use axum::{
    Json,
    http::{HeaderMap, HeaderValue, StatusCode, header::IntoHeaderName},
    response::{IntoResponse, Response},
};
use esruntime_sdk::error::{CommandError, ErrorCode, ExecuteError, SerializationError};
use serde::Serialize;
use umadb_dcb::DCBError;

pub struct Error {
    status_code: StatusCode,
    headers: HeaderMap,
    status: ErrorStatus,
    code: String,
    message: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ErrorStatus {
    /// Client sent invalid/malformed input (400)
    InvalidInput,

    /// Authentication required or failed (401)
    Unauthorized,

    /// Client lacks permission (403)
    Forbidden,

    /// Resource not found (404)
    NotFound,

    /// Request conflicts with current state (409)
    Conflict,

    /// Valid request but business rules rejected it (422)
    Rejected,

    /// Server-side error (500)
    Internal,

    /// Service temporarily unavailable (503)
    Unavailable,
}

impl ErrorStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            ErrorStatus::InvalidInput => "invalid_input",
            ErrorStatus::Unauthorized => "unauthorized",
            ErrorStatus::Forbidden => "forbidden",
            ErrorStatus::NotFound => "not_found",
            ErrorStatus::Conflict => "conflict",
            ErrorStatus::Rejected => "rejected",
            ErrorStatus::Internal => "internal",
            ErrorStatus::Unavailable => "unavailable",
        }
    }

    pub fn status_code(&self) -> StatusCode {
        match self {
            ErrorStatus::InvalidInput => StatusCode::BAD_REQUEST,
            ErrorStatus::Unauthorized => StatusCode::UNAUTHORIZED,
            ErrorStatus::Forbidden => StatusCode::FORBIDDEN,
            ErrorStatus::NotFound => StatusCode::NOT_FOUND,
            ErrorStatus::Conflict => StatusCode::CONFLICT,
            ErrorStatus::Rejected => StatusCode::UNPROCESSABLE_ENTITY,
            ErrorStatus::Internal => StatusCode::INTERNAL_SERVER_ERROR,
            ErrorStatus::Unavailable => StatusCode::SERVICE_UNAVAILABLE,
        }
    }
}

impl fmt::Display for ErrorStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl Error {
    pub fn new(status: ErrorStatus, code: impl Into<String>) -> Self {
        Error {
            status_code: status.status_code(),
            headers: HeaderMap::new(),
            status,
            code: code.into(),
            message: None,
        }
    }

    pub fn with_status_code(mut self, status_code: StatusCode) -> Self {
        self.status_code = status_code;
        self
    }

    pub fn with_header(mut self, key: impl IntoHeaderName, val: impl Into<HeaderValue>) -> Self {
        self.headers.insert(key, val.into());
        self
    }

    pub fn with_message(mut self, msg: impl Into<String>) -> Self {
        self.message = Some(msg.into());
        self
    }
}

impl IntoResponse for Error {
    fn into_response(self) -> Response {
        #[derive(Serialize)]
        struct Body {
            status: &'static str,
            code: String,
            #[serde(skip_serializing_if = "Option::is_none")]
            message: Option<String>,
        }

        (
            self.status_code,
            self.headers,
            Json(Body {
                status: self.status.as_str(),
                code: self.code,
                message: self.message,
            }),
        )
            .into_response()
    }
}

impl<E: std::error::Error> From<ExecuteError<E>> for Error {
    fn from(err: ExecuteError<E>) -> Self {
        match err {
            ExecuteError::Command(err) => {
                Error::new(ErrorStatus::Rejected, "command_rejected").with_message(err.to_string())
            }
            ExecuteError::Validation(err) => {
                Error::new(ErrorStatus::Rejected, "command_rejected").with_message(err.to_string())
            }
            ExecuteError::DCB(err) => err.into(),
            ExecuteError::Serialization(err) => err.into(),
        }
    }
}

impl From<CommandError> for Error {
    fn from(err: CommandError) -> Self {
        let status = match err.code {
            ErrorCode::Rejected => ErrorStatus::Rejected,
            ErrorCode::InvalidInput => ErrorStatus::InvalidInput,
            ErrorCode::Internal => ErrorStatus::Internal,
        };

        Error::new(status, format!("command_{}", err.code)).with_message(err.message)
    }
}

impl From<SerializationError> for Error {
    fn from(err: SerializationError) -> Self {
        Error::new(ErrorStatus::Internal, "serialization_error").with_message(err.message)
    }
}

impl From<DCBError> for Error {
    fn from(err: DCBError) -> Self {
        match err {
            // System/IO errors - Internal Server Error
            DCBError::Io(error) => {
                Error::new(ErrorStatus::Internal, "io_error").with_message(error.to_string())
            }

            // Data integrity/corruption errors - Internal Server Error
            DCBError::IntegrityError(msg) => {
                Error::new(ErrorStatus::Conflict, "integrity_error").with_message(msg)
            }

            DCBError::Corruption(msg) => {
                Error::new(ErrorStatus::Internal, "corruption").with_message(msg)
            }

            DCBError::DatabaseCorrupted(msg) => {
                Error::new(ErrorStatus::Internal, "database_corrupted").with_message(msg)
            }

            // Initialization/setup errors - Service Unavailable
            DCBError::InitializationError(msg) => {
                Error::new(ErrorStatus::Unavailable, "initialization_error").with_message(msg)
            }

            // Internal consistency errors - Internal Server Error
            DCBError::PageNotFound(page_id) => Error::new(ErrorStatus::Internal, "page_not_found")
                .with_message(format!("Page {} not found", page_id)),

            DCBError::DirtyPageNotFound(page_id) => {
                Error::new(ErrorStatus::Internal, "dirty_page_not_found")
                    .with_message(format!("Dirty page {} not found", page_id))
            }

            DCBError::RootIDMismatch(old_id, new_id) => {
                Error::new(ErrorStatus::Internal, "root_id_mismatch").with_message(format!(
                    "Root ID mismatch: expected {}, got {}",
                    old_id, new_id
                ))
            }

            DCBError::PageAlreadyFreed(page_id) => {
                Error::new(ErrorStatus::Internal, "page_already_freed")
                    .with_message(format!("Page {} already freed", page_id))
            }

            DCBError::PageAlreadyDirty(page_id) => {
                Error::new(ErrorStatus::Internal, "page_already_dirty")
                    .with_message(format!("Page {} already dirty", page_id))
            }

            DCBError::InternalError(msg) => {
                Error::new(ErrorStatus::Internal, "internal_error").with_message(msg)
            }

            // Serialization errors - Internal Server Error
            DCBError::SerializationError(msg) => {
                Error::new(ErrorStatus::Internal, "serialization_error").with_message(msg)
            }

            DCBError::DeserializationError(msg) => {
                Error::new(ErrorStatus::Internal, "deserialization_error").with_message(msg)
            }

            // Transport/network errors - Service Unavailable
            DCBError::TransportError(msg) => {
                Error::new(ErrorStatus::Unavailable, "transport_error").with_message(msg)
            }

            // User cancellation - 499 Client Closed Request
            // Keep manual override since there's no standard status for this
            DCBError::CancelledByUser() => Error::new(ErrorStatus::Internal, "cancelled")
                .with_status_code(StatusCode::from_u16(499).unwrap_or(StatusCode::REQUEST_TIMEOUT))
                .with_message("Request cancelled by user"),

            // Authentication errors - Unauthorized
            DCBError::AuthenticationError(msg) => {
                Error::new(ErrorStatus::Unauthorized, "authentication_error").with_message(msg)
            }
        }
    }
}
