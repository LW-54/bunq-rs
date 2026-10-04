use std::time::Duration;

use reqwest::{
    Client, Url,
    header::{CACHE_CONTROL, CONTENT_TYPE, HeaderMap, HeaderName, HeaderValue, USER_AGENT},
};
use uuid::Uuid;

use crate::{
    ClientConfig, Error, Method, PrivateKey, PublicKey, Result, decode_error, sign, verify,
};

const AUTHENTICATION_HEADER: HeaderName = HeaderName::from_static("x-bunq-client-authentication");
const REQUEST_ID_HEADER: HeaderName = HeaderName::from_static("x-bunq-client-request-id");
const SIGNATURE_HEADER: HeaderName = HeaderName::from_static("x-bunq-client-signature");
const SERVER_SIGNATURE_HEADER: HeaderName = HeaderName::from_static("x-bunq-server-signature");
const RESPONSE_ID_HEADER: HeaderName = HeaderName::from_static("x-bunq-client-response-id");

/// The raw response returned by bunq after transport and signature checks.
#[derive(Debug)]
pub struct Response {
    pub status: u16,
    pub request_id: String,
    pub response_request_id: Option<String>,
    pub response_id: Option<String>,
    pub body: Vec<u8>,
}

/// Configures a [`Transport`].
#[derive(Clone, Debug)]
pub struct TransportBuilder {
    config: ClientConfig,
    timeout: Duration,
}

impl TransportBuilder {
    /// Creates a builder with a ten-second request timeout.
    #[must_use]
    pub const fn new(config: ClientConfig) -> Self {
        Self {
            config,
            timeout: Duration::from_secs(10),
        }
    }

    /// Sets the request timeout.
    #[must_use]
    pub const fn timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    /// Builds a transport using the provided client signing key.
    ///
    /// # Errors
    ///
    /// Returns an error if reqwest cannot construct the configured HTTP client.
    pub fn build(self, private_key: PrivateKey) -> Result<Transport> {
        let http_client = Client::builder().timeout(self.timeout).build()?;
        Ok(Transport {
            config: self.config,
            http_client,
            private_key,
            server_key: None,
            authentication_token: None,
        })
    }
}

/// The shared HTTP transport for authenticated and installation requests.
pub struct Transport {
    config: ClientConfig,
    http_client: Client,
    private_key: PrivateKey,
    server_key: Option<PublicKey>,
    authentication_token: Option<String>,
}

impl std::fmt::Debug for Transport {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Transport")
            .field("config", &self.config)
            .field("server_key", &self.server_key)
            .field(
                "authentication_token",
                &self.authentication_token.as_ref().map(|_| "REDACTED"),
            )
            .finish_non_exhaustive()
    }
}

impl Transport {
    /// Sets the token sent in `X-Bunq-Client-Authentication`.
    pub fn set_authentication_token(&mut self, token: Option<String>) {
        self.authentication_token = token;
    }

    /// Sets the server key used to verify signed responses.
    pub fn set_server_key(&mut self, key: Option<PublicKey>) {
        self.server_key = key;
    }

    /// Sends a request without requiring a response signature.
    ///
    /// # Errors
    ///
    /// Returns an error for invalid endpoints, request construction or network failures, malformed
    /// bunq errors, and non-success HTTP responses.
    pub async fn send_unverified(
        &self,
        method: Method,
        endpoint: &str,
        body: Option<Vec<u8>>,
        sign_body: bool,
    ) -> Result<Response> {
        self.send(method, endpoint, body, sign_body, false).await
    }

    /// Sends a request and verifies its bunq server signature.
    ///
    /// # Errors
    ///
    /// Returns an error for invalid endpoints, request construction or network failures, missing or
    /// invalid response signatures, malformed bunq errors, and non-success HTTP responses.
    #[allow(clippy::too_many_lines)]
    pub async fn send(
        &self,
        method: Method,
        endpoint: &str,
        body: Option<Vec<u8>>,
        sign_body: bool,
        require_response_signature: bool,
    ) -> Result<Response> {
        let url = self.endpoint_url(endpoint)?;
        let request_id = Uuid::new_v4().to_string();
        let body = body.unwrap_or_default();
        let mut headers = HeaderMap::new();
        headers.insert(
            USER_AGENT,
            HeaderValue::try_from(self.config.user_agent())
                .map_err(|error| Error::InvalidConfiguration(error.to_string()))?,
        );
        headers.insert(CACHE_CONTROL, HeaderValue::from_static("no-cache"));
        headers.insert(
            REQUEST_ID_HEADER,
            HeaderValue::try_from(&request_id)
                .map_err(|error| Error::InvalidConfiguration(error.to_string()))?,
        );
        if !body.is_empty() {
            headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
        }
        if let Some(token) = &self.authentication_token {
            headers.insert(
                AUTHENTICATION_HEADER,
                HeaderValue::try_from(token)
                    .map_err(|error| Error::InvalidConfiguration(error.to_string()))?,
            );
        }
        if sign_body {
            headers.insert(
                SIGNATURE_HEADER,
                HeaderValue::try_from(sign(&self.private_key, &body)?)
                    .map_err(|error| Error::InvalidConfiguration(error.to_string()))?,
            );
        }
        let response = self
            .http_client
            .request(method, url)
            .headers(headers)
            .body(body)
            .send()
            .await?;
        let status = response.status().as_u16();
        if response.content_length().is_some_and(|length| {
            u64::try_from(self.config.max_response_bytes()).is_ok_and(|limit| length > limit)
        }) {
            return Err(Error::ResponseTooLarge {
                limit: self.config.max_response_bytes(),
            });
        }
        let response_id = response
            .headers()
            .get(&RESPONSE_ID_HEADER)
            .and_then(|value| value.to_str().ok())
            .map(str::to_owned);
        let response_request_id = response
            .headers()
            .get(&REQUEST_ID_HEADER)
            .and_then(|value| value.to_str().ok())
            .map(str::to_owned);
        let server_signature = response
            .headers()
            .get(&SERVER_SIGNATURE_HEADER)
            .and_then(|value| value.to_str().ok())
            .map(str::to_owned);
        let body = response.bytes().await?.to_vec();
        if body.len() > self.config.max_response_bytes() {
            return Err(Error::ResponseTooLarge {
                limit: self.config.max_response_bytes(),
            });
        }
        if require_response_signature {
            let key = self
                .server_key
                .as_ref()
                .ok_or(Error::MissingResponseSignature)?;
            let signature = server_signature
                .as_deref()
                .ok_or(Error::MissingResponseSignature)?;
            verify(key, &body, signature)?;
            let echoed_request_id = response_request_id
                .as_deref()
                .ok_or(Error::MissingResponseRequestId)?;
            if echoed_request_id != request_id {
                return Err(Error::ResponseRequestIdMismatch {
                    expected: request_id,
                    actual: echoed_request_id.to_owned(),
                });
            }
        }
        if !(200..300).contains(&status) {
            let error = decode_error(&body)?
                .errors
                .into_iter()
                .next()
                .ok_or_else(|| Error::InvalidResponse("Error array was empty".to_owned()))?;
            return Err(Error::Api {
                status,
                error,
                request_id: Some(request_id.clone()),
                response_request_id,
                response_id,
            });
        }
        Ok(Response {
            status,
            request_id,
            response_request_id,
            response_id,
            body,
        })
    }

    fn endpoint_url(&self, endpoint: &str) -> Result<Url> {
        let mut base = Url::parse(self.config.base_url())
            .map_err(|error| Error::InvalidConfiguration(error.to_string()))?;
        let endpoint = endpoint.trim_start_matches('/');
        if endpoint.is_empty() || endpoint.contains("..") || endpoint.starts_with("http") {
            return Err(Error::InvalidConfiguration(
                "endpoint must be a relative API path".to_owned(),
            ));
        }
        let (path, query) = endpoint.split_once('?').unwrap_or((endpoint, ""));
        if path.is_empty() || path.split('/').any(str::is_empty) {
            return Err(Error::InvalidConfiguration(
                "endpoint contains an invalid path".to_owned(),
            ));
        }
        let base_path = base.path().trim_end_matches('/');
        base.set_path(&format!("{base_path}/{path}"));
        base.set_query((!query.is_empty()).then_some(query));
        Ok(base)
    }
}
