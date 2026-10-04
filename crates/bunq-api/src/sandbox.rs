use reqwest::{
    Client, Method, Url,
    header::{CACHE_CONTROL, CONTENT_TYPE, HeaderName, HeaderValue, USER_AGENT},
};
use serde_json::Value;
use uuid::Uuid;

use crate::{Error, Result};

const SANDBOX_BASE_URL: &str = "https://public-api.sandbox.bunq.com/v1/";
const REQUEST_ID_HEADER: HeaderName = HeaderName::from_static("x-bunq-client-request-id");

/// The type of disposable sandbox user to create.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SandboxUserKind {
    /// Creates a sandbox user representing an individual.
    Person,
    /// Creates a sandbox user representing a business.
    Company,
}

impl SandboxUserKind {
    const fn endpoint(self) -> &'static str {
        match self {
            Self::Person => "sandbox-user-person",
            Self::Company => "sandbox-user-company",
        }
    }
}

/// Creates a disposable sandbox user and returns its API key.
///
/// The sandbox endpoint does not require an existing API key. The returned key
/// must be treated as a credential and should only be stored in a test secret
/// store or process environment.
///
/// # Errors
///
/// Returns an error for HTTP, JSON, malformed bunq envelope, or sandbox API failures.
pub async fn create_sandbox_user(kind: SandboxUserKind) -> Result<String> {
    let client = Client::builder().build().map_err(Error::Request)?;
    let url = Url::parse(SANDBOX_BASE_URL)
        .map_err(|error| Error::InvalidConfiguration(error.to_string()))?
        .join(kind.endpoint())
        .map_err(|error| Error::InvalidConfiguration(error.to_string()))?;
    let request_id = Uuid::new_v4().to_string();
    let response = client
        .request(Method::POST, url)
        .header(USER_AGENT, "bunq-api-sandbox/0.1")
        .header(CACHE_CONTROL, "no-cache")
        .header(CONTENT_TYPE, "application/json")
        .header(
            REQUEST_ID_HEADER,
            HeaderValue::try_from(request_id)
                .map_err(|error| Error::InvalidConfiguration(error.to_string()))?,
        )
        .body(Vec::new())
        .send()
        .await?;
    let status = response.status().as_u16();
    let body = response.bytes().await?.to_vec();
    if !(200..300).contains(&status) {
        let error = crate::decode_error(&body)?
            .errors
            .into_iter()
            .next()
            .ok_or_else(|| Error::InvalidResponse("sandbox Error array was empty".to_owned()))?;
        return Err(Error::Api {
            status,
            error,
            request_id: None,
            response_request_id: None,
            response_id: None,
        });
    }
    parse_sandbox_api_key(&body)
}

fn parse_sandbox_api_key(body: &[u8]) -> Result<String> {
    let root: Value = serde_json::from_slice(body)?;
    let response = root
        .get("Response")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            Error::InvalidResponse("sandbox response is missing Response array".to_owned())
        })?;
    response
        .iter()
        .find_map(|entry| entry.get("ApiKey"))
        .and_then(|api_key| api_key.get("api_key"))
        .and_then(Value::as_str)
        .filter(|api_key| !api_key.trim().is_empty())
        .map(str::to_owned)
        .ok_or_else(|| {
            Error::InvalidResponse("sandbox response is missing ApiKey.api_key".to_owned())
        })
}

#[cfg(test)]
mod tests {
    use super::{SandboxUserKind, parse_sandbox_api_key};

    #[test]
    fn uses_documented_sandbox_endpoints() {
        assert_eq!(SandboxUserKind::Person.endpoint(), "sandbox-user-person");
        assert_eq!(SandboxUserKind::Company.endpoint(), "sandbox-user-company");
    }

    #[test]
    fn parses_sandbox_api_key() {
        let body = br#"{"Response":[{"ApiKey":{"api_key":"sandbox_key"}}]}"#;
        assert_eq!(parse_sandbox_api_key(body).expect("API key"), "sandbox_key");
    }

    #[test]
    fn rejects_missing_or_empty_sandbox_api_key() {
        assert!(parse_sandbox_api_key(br#"{"Response":[]}"#).is_err());
        assert!(parse_sandbox_api_key(br#"{"Response":[{"ApiKey":{"api_key":""}}]}"#).is_err());
    }
}
