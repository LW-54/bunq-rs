use reqwest::Method;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::{ClientConfig, Error, PrivateKey, PublicKey, Result, TransportBuilder};

/// Credentials and device state returned by the installation flow.
#[derive(Clone)]
pub struct InstallationContext {
    api_base_url: String,
    user_agent: String,
    api_key: String,
    installation_token: String,
    registered_device_id: u64,
    client_private_key_pem: String,
    server_public_key_pem: String,
}

/// Explicit persisted representation of an installed device.
///
/// This record contains credentials and private key material. It is not encrypted by the SDK;
/// callers must store it in an appropriately protected secret store.
#[derive(Clone, Deserialize, Serialize)]
pub struct PersistedInstallation {
    api_base_url: String,
    user_agent: String,
    api_key: String,
    installation_token: String,
    registered_device_id: u64,
    client_private_key_pem: String,
    server_public_key_pem: String,
}

/// A session token and the bunq user it authenticates.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Session {
    token: String,
    user_id: u64,
}

impl Session {
    /// Returns the session token.
    #[must_use]
    pub fn token(&self) -> &str {
        &self.token
    }

    /// Returns the authenticated bunq user ID.
    #[must_use]
    pub const fn user_id(&self) -> u64 {
        self.user_id
    }
}

impl std::fmt::Debug for InstallationContext {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("InstallationContext")
            .field("api_base_url", &self.api_base_url)
            .field("user_agent", &self.user_agent)
            .field("api_key", &"REDACTED")
            .field("installation_token", &"REDACTED")
            .field("registered_device_id", &self.registered_device_id)
            .field("client_private_key_pem", &"REDACTED")
            .field("server_public_key_pem", &"REDACTED")
            .finish()
    }
}

impl std::fmt::Debug for PersistedInstallation {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("PersistedInstallation")
            .field("api_base_url", &self.api_base_url)
            .field("user_agent", &self.user_agent)
            .field("api_key", &"REDACTED")
            .field("installation_token", &"REDACTED")
            .field("registered_device_id", &self.registered_device_id)
            .field("client_private_key_pem", &"REDACTED")
            .field("server_public_key_pem", &"REDACTED")
            .finish()
    }
}

impl InstallationContext {
    /// Converts runtime state into an explicit credential-bearing persistence record.
    #[must_use]
    pub fn to_persisted(&self) -> PersistedInstallation {
        PersistedInstallation {
            api_base_url: self.api_base_url.clone(),
            user_agent: self.user_agent.clone(),
            api_key: self.api_key.clone(),
            installation_token: self.installation_token.clone(),
            registered_device_id: self.registered_device_id,
            client_private_key_pem: self.client_private_key_pem.clone(),
            server_public_key_pem: self.server_public_key_pem.clone(),
        }
    }

    /// Returns the registered device identifier.
    #[must_use]
    pub const fn registered_device_id(&self) -> u64 {
        self.registered_device_id
    }

    /// Creates a transport authenticated with the installation token.
    ///
    /// # Errors
    ///
    /// Returns an error if persisted key material, configuration, or the HTTP client cannot be built.
    pub fn transport(&self) -> Result<crate::Transport> {
        let private_key = PrivateKey::from_pem(&self.client_private_key_pem)?;
        let server_key = PublicKey::from_pem(&self.server_public_key_pem)?;
        let config = ClientConfig::new(&self.api_base_url, &self.user_agent)?;
        let mut transport = TransportBuilder::new(config).build(private_key)?;
        transport.set_authentication_token(Some(self.installation_token.clone()));
        transport.set_server_key(Some(server_key));
        Ok(transport)
    }

    /// Creates a fresh temporary session from this registered installation.
    ///
    /// # Errors
    ///
    /// Returns an error if persisted credentials are invalid, the session request fails, or bunq
    /// returns an invalid session response.
    pub async fn create_session(&self) -> Result<Session> {
        let transport = self.transport()?;
        let session_body = serde_json::to_vec(&json!({ "secret": self.api_key }))?;
        let response = transport
            .send(
                Method::POST,
                "session-server",
                Some(session_body),
                true,
                true,
            )
            .await?;
        parse_session(&response.body)
    }
}

impl PersistedInstallation {
    /// Serializes the credential-bearing record as JSON.
    ///
    /// # Errors
    ///
    /// Returns an error if JSON serialization fails.
    pub fn to_json(&self) -> Result<String> {
        serde_json::to_string(self).map_err(Error::Serialization)
    }

    /// Parses a credential-bearing record from JSON and validates its key material and configuration.
    ///
    /// # Errors
    ///
    /// Returns an error for invalid JSON, configuration, or PEM key material.
    pub fn from_json(json: &str) -> Result<Self> {
        let persisted = serde_json::from_str::<Self>(json).map_err(Error::Serialization)?;
        ClientConfig::new(&persisted.api_base_url, &persisted.user_agent)?;
        PrivateKey::from_pem(&persisted.client_private_key_pem)?;
        PublicKey::from_pem(&persisted.server_public_key_pem)?;
        Ok(persisted)
    }

    /// Reconstructs runtime installation state from this persisted record.
    ///
    /// # Errors
    ///
    /// Returns an error for invalid configuration or key material.
    pub fn into_context(self) -> Result<InstallationContext> {
        ClientConfig::new(&self.api_base_url, &self.user_agent)?;
        PrivateKey::from_pem(&self.client_private_key_pem)?;
        PublicKey::from_pem(&self.server_public_key_pem)?;
        Ok(InstallationContext {
            api_base_url: self.api_base_url,
            user_agent: self.user_agent,
            api_key: self.api_key,
            installation_token: self.installation_token,
            registered_device_id: self.registered_device_id,
            client_private_key_pem: self.client_private_key_pem,
            server_public_key_pem: self.server_public_key_pem,
        })
    }
}

/// Performs installation, device registration, and session creation.
///
/// # Errors
///
/// Returns an error for invalid configuration, key generation or parsing failures, network failures,
/// invalid bunq responses, signature failures, or bunq API errors.
pub async fn install_device(
    api_key: impl Into<String>,
    base_url: impl Into<String>,
    user_agent: impl Into<String>,
    device_description: impl Into<String>,
) -> Result<(InstallationContext, Session)> {
    let api_key = api_key.into();
    if api_key.trim().is_empty() {
        return Err(Error::InvalidConfiguration(
            "API key must not be empty".to_owned(),
        ));
    }
    let base_url = base_url.into();
    let user_agent = user_agent.into();
    let config = ClientConfig::new(&base_url, &user_agent)?;
    let (private_key, client_public_key) = crate::generate_key_pair()?;
    let client_public_key_pem = client_public_key.to_pem()?;
    let mut transport = TransportBuilder::new(config).build(private_key.clone())?;
    let installation_body = serde_json::to_vec(&json!({
        "client_public_key": client_public_key_pem,
    }))?;
    let installation_response = transport
        .send_unverified(Method::POST, "installation", Some(installation_body), false)
        .await?;
    let installation = parse_installation(&installation_response.body)?;
    let server_key = PublicKey::from_pem(&installation.server_public_key_pem)?;
    transport.set_server_key(Some(server_key));
    transport.set_authentication_token(Some(installation.installation_token.clone()));

    let device_body = serde_json::to_vec(&json!({
        "description": device_description.into(),
        "secret": api_key.clone(),
    }))?;
    let device_response = transport
        .send(
            Method::POST,
            "device-server",
            Some(device_body),
            false,
            true,
        )
        .await?;
    let registered_device_id = parse_id(&device_response.body, "device-server")?;

    let session_body = serde_json::to_vec(&json!({ "secret": api_key }))?;
    let session_response = transport
        .send(
            Method::POST,
            "session-server",
            Some(session_body),
            true,
            true,
        )
        .await?;
    let session = parse_session(&session_response.body)?;
    let client_private_key_pem = private_key.to_pem()?;
    let context = InstallationContext {
        api_base_url: base_url,
        user_agent,
        api_key,
        installation_token: installation.installation_token,
        registered_device_id,
        client_private_key_pem,
        server_public_key_pem: installation.server_public_key_pem,
    };
    Ok((context, session))
}

struct InstallationData {
    installation_token: String,
    server_public_key_pem: String,
}

fn parse_installation(body: &[u8]) -> Result<InstallationData> {
    let root: Value = serde_json::from_slice(body)?;
    let response = root
        .get("Response")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            Error::InvalidResponse("installation response is missing Response array".to_owned())
        })?;
    let installation_token = find_string(response, "Token", "token")?;
    let server_public_key_pem = find_string(response, "ServerPublicKey", "server_public_key")?;
    Ok(InstallationData {
        installation_token,
        server_public_key_pem,
    })
}

fn parse_session(body: &[u8]) -> Result<Session> {
    let root: Value = serde_json::from_slice(body)?;
    let response = root
        .get("Response")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            Error::InvalidResponse("session-server response is missing Response array".to_owned())
        })?;
    let token = find_string(response, "Token", "token")?;
    let user_id = response
        .iter()
        .find_map(|entry| entry.get("UserPerson").or_else(|| entry.get("UserCompany")))
        .and_then(|user| user.get("id"))
        .and_then(Value::as_u64)
        .ok_or_else(|| {
            Error::InvalidResponse("session-server response is missing user ID".to_owned())
        })?;
    Ok(Session { token, user_id })
}

fn parse_id(body: &[u8], endpoint: &str) -> Result<u64> {
    let root: Value = serde_json::from_slice(body)?;
    let response = root
        .get("Response")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            Error::InvalidResponse(format!("{endpoint} response is missing Response array"))
        })?;
    let id = response
        .iter()
        .find_map(|entry| entry.get("Id"))
        .and_then(|entry| entry.get("id"))
        .and_then(Value::as_u64);
    id.ok_or_else(|| Error::InvalidResponse(format!("{endpoint} response is missing numeric Id")))
}

fn find_string(response: &[Value], object_key: &str, field: &str) -> Result<String> {
    response
        .iter()
        .find_map(|entry| entry.get(object_key))
        .and_then(|object| object.get(field))
        .and_then(Value::as_str)
        .map(str::to_owned)
        .ok_or_else(|| Error::InvalidResponse(format!("response is missing {object_key}.{field}")))
}

#[cfg(test)]
mod tests {
    use super::{
        InstallationContext, PersistedInstallation, parse_id, parse_installation, parse_session,
    };

    #[test]
    fn persists_installation_explicitly_and_redacts_secrets() {
        let (private_key, public_key) = crate::generate_key_pair().expect("keys");
        let context = InstallationContext {
            api_base_url: "https://api.example/v1".to_owned(),
            user_agent: "test/1.0".to_owned(),
            api_key: "secret-api-key".to_owned(),
            installation_token: "secret-installation-token".to_owned(),
            registered_device_id: 7,
            client_private_key_pem: private_key.to_pem().expect("private PEM"),
            server_public_key_pem: public_key.to_pem().expect("public PEM"),
        };
        let persisted = context.to_persisted();
        let json = persisted.to_json().expect("persisted JSON");
        let restored = PersistedInstallation::from_json(&json).expect("persisted installation");
        let restored_context = restored.into_context().expect("runtime context");
        assert_eq!(restored_context.registered_device_id(), 7);
        let debug = format!("{restored_context:?} {persisted:?}");
        assert!(!debug.contains("secret-api-key"));
        assert!(!debug.contains("secret-installation-token"));
    }

    #[test]
    fn rejects_invalid_persisted_key_material() {
        let json = r#"{
            "api_base_url":"https://api.example/v1",
            "user_agent":"test/1.0",
            "api_key":"key",
            "installation_token":"token",
            "registered_device_id":7,
            "client_private_key_pem":"bad",
            "server_public_key_pem":"bad"
        }"#;
        assert!(PersistedInstallation::from_json(json).is_err());
    }

    #[test]
    fn parses_heterogeneous_installation_response_without_order_assumptions() {
        let body = br#"{
            "Response": [
                {"ServerPublicKey": {"server_public_key": "server-key"}},
                {"Token": {"token": "installation-token"}},
                {"Id": {"id": 12}}
            ]
        }"#;
        let installation = parse_installation(body).expect("installation");
        assert_eq!(installation.installation_token, "installation-token");
        assert_eq!(installation.server_public_key_pem, "server-key");
    }

    #[test]
    fn rejects_missing_installation_fields() {
        assert!(parse_installation(br#"{"Response":[{"Token":{"token":"x"}}]}"#).is_err());
        assert!(parse_installation(br#"{"Response":[{"ServerPublicKey":{}}]}"#).is_err());
    }

    #[test]
    fn rejects_missing_or_non_numeric_device_id() {
        assert!(parse_id(br#"{"Response":[]}"#, "device-server").is_err());
        assert!(parse_id(br#"{"Response":[{"Id":{"id":"12"}}]}"#, "device-server").is_err());
    }

    #[test]
    fn parses_session_token_and_user_id() {
        let session = parse_session(
            br#"{"Response":[{"Token":{"token":"session-token"}},{"UserPerson":{"id":42}}]}"#,
        )
        .expect("session");
        assert_eq!(session.token(), "session-token");
        assert_eq!(session.user_id(), 42);
    }

    #[test]
    fn rejects_session_without_user_id() {
        assert!(parse_session(br#"{"Response":[{"Token":{"token":"session-token"}}]}"#).is_err());
    }
}
