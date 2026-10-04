use std::fmt;

use thiserror::Error as ThisError;

use crate::envelope::ApiError;

/// Result type used by the SDK.
pub type Result<T> = std::result::Result<T, Error>;

/// Errors returned by the SDK.
#[derive(Debug, ThisError)]
#[non_exhaustive]
pub enum Error {
    #[error("invalid client configuration: {0}")]
    InvalidConfiguration(String),
    #[error("invalid API response: {0}")]
    InvalidResponse(String),
    #[error("API response exceeds configured limit of {limit} bytes")]
    ResponseTooLarge { limit: usize },
    #[error("bunq response signature is missing")]
    MissingResponseSignature,
    #[error("bunq response request ID is missing")]
    MissingResponseRequestId,
    #[error("bunq response request ID mismatch: expected {expected}, received {actual}")]
    ResponseRequestIdMismatch { expected: String, actual: String },
    #[error("failed to serialize JSON: {0}")]
    Serialization(#[from] serde_json::Error),
    #[error("request failed: {0}")]
    Request(#[from] reqwest::Error),
    #[error("cryptographic operation failed: {0}")]
    Crypto(String),
    #[error("bunq API returned HTTP {status}: {error}")]
    Api {
        status: u16,
        error: ApiError,
        request_id: Option<String>,
        response_request_id: Option<String>,
        response_id: Option<String>,
    },
}

impl Error {
    /// Returns the HTTP status when bunq returned an API response error.
    #[must_use]
    pub const fn status(&self) -> Option<u16> {
        match self {
            Self::Api { status, .. } => Some(*status),
            _ => None,
        }
    }

    /// Returns the client request ID associated with an API response error.
    #[must_use]
    pub fn request_id(&self) -> Option<&str> {
        match self {
            Self::Api { request_id, .. } => request_id.as_deref(),
            _ => None,
        }
    }

    /// Returns the echoed request ID from an API response error.
    #[must_use]
    pub fn response_request_id(&self) -> Option<&str> {
        match self {
            Self::Api {
                response_request_id,
                ..
            } => response_request_id.as_deref(),
            _ => None,
        }
    }

    /// Returns bunq's response ID from an API response error.
    #[must_use]
    pub fn response_id(&self) -> Option<&str> {
        match self {
            Self::Api { response_id, .. } => response_id.as_deref(),
            _ => None,
        }
    }

    /// Returns whether bunq rejected the request because of rate limiting.
    #[must_use]
    pub const fn is_rate_limited(&self) -> bool {
        matches!(self, Self::Api { status: 429, .. })
    }

    /// Returns whether the error represents an authentication or signature failure.
    #[must_use]
    pub const fn is_authentication_failure(&self) -> bool {
        matches!(
            self,
            Self::Api {
                status: 401 | 466,
                ..
            }
        )
    }

    /// Returns whether the server reported a 5xx failure.
    #[must_use]
    pub const fn is_server_failure(&self) -> bool {
        matches!(self, Self::Api { status, .. } if *status >= 500 && *status <= 599)
    }

    /// Returns whether a bounded retry may be appropriate for an idempotent operation.
    ///
    /// This is deliberately false for payment writes. The SDK does not retry operations
    /// automatically; callers must also consider whether their operation is safe to repeat.
    #[must_use]
    pub const fn is_retryable_transport_error(&self) -> bool {
        match self {
            Self::Request(_) => true,
            Self::Api { status, .. } => *status == 429 || (*status >= 500 && *status <= 599),
            _ => false,
        }
    }
}

impl From<base64::DecodeError> for Error {
    fn from(error: base64::DecodeError) -> Self {
        Self::InvalidResponse(format!("invalid base64: {error}"))
    }
}

impl fmt::Display for ApiError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.error_description_translated.is_empty() {
            formatter.write_str(&self.error_description)
        } else {
            write!(
                formatter,
                "{} ({})",
                self.error_description, self.error_description_translated
            )
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{ApiError, Error};

    fn api(status: u16) -> Error {
        Error::Api {
            status,
            error: ApiError {
                error_description: "test".to_owned(),
                error_description_translated: String::new(),
            },
            request_id: None,
            response_request_id: None,
            response_id: None,
        }
    }

    #[test]
    fn classifies_operational_statuses() {
        assert!(api(429).is_rate_limited());
        assert!(api(429).is_retryable_transport_error());
        assert!(api(401).is_authentication_failure());
        assert!(api(466).is_authentication_failure());
        assert!(api(503).is_server_failure());
        assert!(api(503).is_retryable_transport_error());
        assert_eq!(api(400).status(), Some(400));
        assert!(!api(400).is_retryable_transport_error());
    }
}
