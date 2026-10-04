use serde::{Deserialize, Serialize, de::DeserializeOwned};

use crate::{Error, Result};

/// A bunq API error entry.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ApiError {
    pub error_description: String,
    #[serde(default)]
    pub error_description_translated: String,
}

/// A decoded bunq error envelope.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ErrorEnvelope {
    #[serde(rename = "Error")]
    pub errors: Vec<ApiError>,
}

/// A decoded bunq response envelope containing raw response entries.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ResponseEnvelope<T> {
    #[serde(rename = "Response")]
    pub response: Vec<T>,
}

/// Decodes a successful bunq response envelope.
///
/// # Errors
///
/// Returns an error for invalid JSON, an invalid response shape, or an empty response array.
pub fn decode_response<T: DeserializeOwned>(body: &[u8]) -> Result<ResponseEnvelope<T>> {
    let envelope = serde_json::from_slice::<ResponseEnvelope<T>>(body)?;
    if envelope.response.is_empty() {
        return Err(Error::InvalidResponse(
            "Response array must contain at least one item".to_owned(),
        ));
    }
    Ok(envelope)
}

/// Decodes a bunq error envelope.
///
/// # Errors
///
/// Returns an error for invalid JSON, an invalid error shape, or an empty error array.
pub fn decode_error(body: &[u8]) -> Result<ErrorEnvelope> {
    let envelope = serde_json::from_slice::<ErrorEnvelope>(body)?;
    if envelope.errors.is_empty() {
        return Err(Error::InvalidResponse(
            "Error array must contain at least one item".to_owned(),
        ));
    }
    Ok(envelope)
}

#[cfg(test)]
mod tests {
    use super::{decode_error, decode_response};

    #[test]
    fn decodes_response_array() {
        let envelope = decode_response::<serde_json::Value>(br#"{"Response":[{"User":{"id":7}}]}"#)
            .expect("response");
        assert_eq!(envelope.response.len(), 1);
    }

    #[test]
    fn decodes_error_array() {
        let envelope =
            decode_error(br#"{"Error":[{"error_description":"bad request"}]}"#).expect("error");
        assert_eq!(envelope.errors[0].error_description, "bad request");
    }

    #[test]
    fn rejects_empty_envelopes() {
        assert!(decode_response::<serde_json::Value>(br#"{"Response":[]}"#).is_err());
        assert!(decode_error(br#"{"Error":[]}"#).is_err());
    }
}
