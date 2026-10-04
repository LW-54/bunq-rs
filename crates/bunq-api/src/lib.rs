#![doc = "A typed, fallible Rust client for the bunq API."]

mod auth;
mod client;
mod envelope;
mod error;
mod resources;
mod sandbox;
mod signing;
mod transport;

pub use auth::{InstallationContext, PersistedInstallation, Session, install_device};
pub use client::Client;
pub use envelope::{ApiError, ErrorEnvelope, ResponseEnvelope, decode_error, decode_response};
pub use error::{Error, Result};
pub use reqwest::Method;
pub use resources::{
    Balance, Counterparty, CounterpartyAlias, CreatedPayment, CreatedRequestInquiry,
    MonetaryAccountBank, MonetaryAccountExternal, MonetaryAccountSavings, Money, PaginatedResponse,
    Pagination, Payment, PaymentBatch, PaymentBatchRequest, PaymentRequest, RequestInquiry,
    RequestInquiryRequest, User, UserCompany, UserPerson,
};
pub use sandbox::{SandboxUserKind, create_sandbox_user};
pub use signing::{PrivateKey, PublicKey, generate_key_pair, sign, verify};
pub use transport::{Response, Transport, TransportBuilder};

/// Configuration shared by all bunq API clients.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClientConfig {
    base_url: String,
    user_agent: String,
    max_response_bytes: usize,
}

const DEFAULT_MAX_RESPONSE_BYTES: usize = 8 * 1024 * 1024;

impl ClientConfig {
    /// Creates a configuration after validating the API base URL.
    ///
    /// # Errors
    ///
    /// Returns an error when the URL does not use HTTP(S) or the user agent is empty.
    pub fn new(base_url: impl Into<String>, user_agent: impl Into<String>) -> Result<Self> {
        let base_url = base_url.into();
        if !base_url.starts_with("https://") {
            return Err(Error::InvalidConfiguration(
                "base URL must use https".to_owned(),
            ));
        }
        let user_agent = user_agent.into();
        if user_agent.trim().is_empty() {
            return Err(Error::InvalidConfiguration(
                "user agent must not be empty".to_owned(),
            ));
        }
        Ok(Self {
            base_url,
            user_agent,
            max_response_bytes: DEFAULT_MAX_RESPONSE_BYTES,
        })
    }

    /// Creates a configuration for local HTTP test servers.
    ///
    /// This constructor must not be used with production credentials or real bunq traffic.
    ///
    /// # Errors
    ///
    /// Returns an error when the URL does not use HTTP or the user agent is empty.
    pub fn new_insecure_http(
        base_url: impl Into<String>,
        user_agent: impl Into<String>,
    ) -> Result<Self> {
        let base_url = base_url.into();
        if !base_url.starts_with("http://") {
            return Err(Error::InvalidConfiguration(
                "insecure test URL must use http".to_owned(),
            ));
        }
        let user_agent = user_agent.into();
        if user_agent.trim().is_empty() {
            return Err(Error::InvalidConfiguration(
                "user agent must not be empty".to_owned(),
            ));
        }
        Ok(Self {
            base_url,
            user_agent,
            max_response_bytes: DEFAULT_MAX_RESPONSE_BYTES,
        })
    }

    /// Sets the maximum response body size accepted by the transport.
    ///
    /// # Errors
    ///
    /// Returns an error when the limit is zero.
    pub fn with_max_response_bytes(mut self, limit: usize) -> Result<Self> {
        if limit == 0 {
            return Err(Error::InvalidConfiguration(
                "maximum response size must be greater than zero".to_owned(),
            ));
        }
        self.max_response_bytes = limit;
        Ok(self)
    }

    /// Returns the configured API base URL.
    #[must_use]
    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    /// Returns the configured user agent.
    #[must_use]
    pub fn user_agent(&self) -> &str {
        &self.user_agent
    }

    /// Returns the configured maximum response body size.
    #[must_use]
    pub const fn max_response_bytes(&self) -> usize {
        self.max_response_bytes
    }
}

#[cfg(test)]
mod tests {
    use super::{ClientConfig, Error};

    #[test]
    fn rejects_invalid_configuration() {
        assert!(matches!(
            ClientConfig::new("bunq.example", "bunq-api/0.1"),
            Err(Error::InvalidConfiguration(_))
        ));
        assert!(matches!(
            ClientConfig::new("https://bunq.example", " "),
            Err(Error::InvalidConfiguration(_))
        ));
        assert!(matches!(
            ClientConfig::new("http://localhost:1234", "bunq-api-test/1.0"),
            Err(Error::InvalidConfiguration(_))
        ));
        assert!(
            ClientConfig::new_insecure_http("http://localhost:1234", "bunq-api-test/1.0").is_ok()
        );
        assert!(
            ClientConfig::new_insecure_http("http://localhost:1234", "test")
                .expect("config")
                .with_max_response_bytes(0)
                .is_err()
        );
    }
}
