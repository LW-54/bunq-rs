#![allow(clippy::expect_used, clippy::missing_panics_doc)]

use std::sync::OnceLock;

use bunq_api::{PrivateKey, PublicKey, generate_key_pair};

type KeyPair = (PrivateKey, PublicKey);

static TEST_KEYS: OnceLock<KeyPair> = OnceLock::new();

pub fn client_keys() -> (PrivateKey, PublicKey) {
    TEST_KEYS
        .get_or_init(|| generate_key_pair().expect("client test keys"))
        .clone()
}

pub fn server_keys() -> (PrivateKey, PublicKey) {
    client_keys()
}
