use base64::{Engine as _, engine::general_purpose::STANDARD};
use openssl::{
    hash::MessageDigest,
    pkey::{PKey, Private, Public},
    rsa::Rsa,
    sign::{Signer, Verifier},
};

use crate::{Error, Result};

/// An RSA private key used to sign exact request bytes.
#[derive(Clone)]
pub struct PrivateKey(PKey<Private>);

impl std::fmt::Debug for PrivateKey {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("PrivateKey(REDACTED)")
    }
}

/// An RSA public key used to verify bunq response signatures.
#[derive(Clone)]
pub struct PublicKey(PKey<Public>);

impl std::fmt::Debug for PublicKey {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("PublicKey(REDACTED)")
    }
}

impl PrivateKey {
    pub(crate) fn to_pem(&self) -> Result<String> {
        self.0
            .private_key_to_pem_pkcs8()
            .map(String::from_utf8)
            .map_err(|error| Error::Crypto(error.to_string()))?
            .map_err(|error| Error::Crypto(error.to_string()))
    }

    /// Parses a PKCS#8 PEM private key.
    ///
    /// # Errors
    ///
    /// Returns an error when the PEM does not contain a valid private key.
    pub fn from_pem(pem: &str) -> Result<Self> {
        PKey::private_key_from_pem(pem.as_bytes())
            .map(Self)
            .map_err(|error| Error::Crypto(error.to_string()))
    }
}

impl PublicKey {
    pub(crate) fn to_pem(&self) -> Result<String> {
        self.0
            .public_key_to_pem()
            .map(String::from_utf8)
            .map_err(|error| Error::Crypto(error.to_string()))?
            .map_err(|error| Error::Crypto(error.to_string()))
    }

    /// Parses a `SubjectPublicKeyInfo` PEM public key.
    ///
    /// # Errors
    ///
    /// Returns an error when the PEM does not contain a valid public key.
    pub fn from_pem(pem: &str) -> Result<Self> {
        PKey::public_key_from_pem(pem.as_bytes())
            .map(Self)
            .map_err(|error| Error::Crypto(error.to_string()))
    }
}

/// Generates a 2048-bit RSA key pair.
///
/// # Errors
///
/// Returns an error if the operating system cannot provide secure randomness or key generation fails.
pub fn generate_key_pair() -> Result<(PrivateKey, PublicKey)> {
    let rsa = Rsa::generate(2048).map_err(|error| Error::Crypto(error.to_string()))?;
    let private = PKey::from_rsa(rsa).map_err(|error| Error::Crypto(error.to_string()))?;
    let public_pem = private
        .public_key_to_pem()
        .map_err(|error| Error::Crypto(error.to_string()))?;
    let public =
        PKey::public_key_from_pem(&public_pem).map_err(|error| Error::Crypto(error.to_string()))?;
    Ok((PrivateKey(private), PublicKey(public)))
}

/// Signs the exact body bytes with RSA-SHA256 and returns a Base64 value.
///
/// # Errors
///
/// This operation currently returns an error only if the signing backend rejects the key.
pub fn sign(private_key: &PrivateKey, body: &[u8]) -> Result<String> {
    let mut signer = Signer::new(MessageDigest::sha256(), &private_key.0)
        .map_err(|error| Error::Crypto(error.to_string()))?;
    signer
        .update(body)
        .map_err(|error| Error::Crypto(error.to_string()))?;
    signer
        .sign_to_vec()
        .map(|signature| STANDARD.encode(signature))
        .map_err(|error| Error::Crypto(error.to_string()))
}

/// Verifies a Base64 RSA-SHA256 signature against the exact body bytes.
///
/// # Errors
///
/// Returns an error if the signature is not valid Base64, has an invalid RSA shape, or does not verify.
pub fn verify(public_key: &PublicKey, body: &[u8], signature: &str) -> Result<()> {
    let signature = STANDARD.decode(signature)?;
    let mut verifier = Verifier::new(MessageDigest::sha256(), &public_key.0)
        .map_err(|error| Error::Crypto(error.to_string()))?;
    verifier
        .update(body)
        .map_err(|error| Error::Crypto(error.to_string()))?;
    if verifier
        .verify(&signature)
        .map_err(|error| Error::Crypto(error.to_string()))?
    {
        Ok(())
    } else {
        Err(Error::Crypto("signature verification failed".to_owned()))
    }
}

#[cfg(test)]
mod tests {
    use super::{generate_key_pair, sign, verify};

    #[test]
    fn signs_exact_bytes_and_rejects_modified_body() {
        let (private_key, public_key) = generate_key_pair().expect("key generation");
        let body = br#"{"amount":"1.00"}"#;
        let signature = sign(&private_key, body).expect("signing");
        assert!(verify(&public_key, body, &signature).is_ok());
        assert!(verify(&public_key, br#"{"amount":"1.01"}"#, &signature).is_err());
    }

    #[test]
    fn rejects_invalid_base64() {
        let (_, public_key) = generate_key_pair().expect("key generation");
        assert!(verify(&public_key, b"body", "not base64").is_err());
    }
}
