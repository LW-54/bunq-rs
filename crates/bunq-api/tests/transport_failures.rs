#![allow(clippy::expect_used)]

use bunq_api::{ClientConfig, Error, Method, TransportBuilder, sign};
use tokio::io::AsyncWriteExt;
use tokio::net::TcpListener;

#[path = "support/mod.rs"]
mod common;

async fn serve_response(status: u16, headers: &str, body: &[u8]) -> String {
    let listener = TcpListener::bind(("127.0.0.1", 0)).await.expect("listener");
    let address = listener.local_addr().expect("listener address");
    let body = body.to_vec();
    let headers = headers.to_owned();
    tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.expect("request");
        let mut request = [0_u8; 1024];
        let _ = socket.readable().await;
        let _ = socket.try_read(&mut request);
        let response = format!(
            "HTTP/1.1 {status} Test\r\nContent-Length: {}\r\n{headers}\r\n",
            body.len()
        );
        socket
            .write_all(response.as_bytes())
            .await
            .expect("headers");
        socket.write_all(&body).await.expect("body");
    });
    format!("http://{address}/v1")
}

fn error_body(description: &str) -> Vec<u8> {
    format!(
        r#"{{"Error":[{{"error_description":"{description}","error_description_translated":""}}]}}"#
    )
    .into_bytes()
}

fn transport(base_url: String) -> bunq_api::Transport {
    let (private_key, _) = common::client_keys();
    TransportBuilder::new(
        ClientConfig::new_insecure_http(base_url, "failure-test/1.0").expect("config"),
    )
    .build(private_key)
    .expect("transport")
}

#[tokio::test]
async fn rejects_missing_response_signature() {
    let body = br#"{"Response":[{"Id":{"id":1}}]}"#;
    let base_url = serve_response(200, "", body).await;
    let transport = transport(base_url);
    let error = transport
        .send(Method::GET, "test", None, false, true)
        .await
        .expect_err("missing signature should fail");
    assert!(matches!(error, Error::MissingResponseSignature));
}

#[tokio::test]
async fn rejects_invalid_response_signature() {
    let body = br#"{"Response":[{"Id":{"id":1}}]}"#;
    let base_url = serve_response(200, "X-Bunq-Server-Signature: invalid\r\n", body).await;
    let (_, server_public_key) = common::server_keys();
    let mut transport = transport(base_url);
    transport.set_server_key(Some(server_public_key));
    let error = transport
        .send(Method::GET, "test", None, false, true)
        .await
        .expect_err("invalid signature should fail");
    assert!(matches!(
        error,
        Error::InvalidResponse(_) | Error::Crypto(_)
    ));
}

#[tokio::test]
async fn classifies_http_api_errors() {
    for (status, expected) in [(401, "auth"), (429, "rate"), (500, "server")] {
        let base_url = serve_response(status, "", &error_body(expected)).await;
        let transport = transport(base_url);
        let error = transport
            .send_unverified(Method::GET, "test", None, false)
            .await
            .expect_err("API error should fail");
        assert_eq!(error.status(), Some(status));
        assert!(error.request_id().is_some());
        if status == 429 {
            assert!(error.is_rate_limited());
        }
        if status == 401 {
            assert!(error.is_authentication_failure());
        }
        if status == 500 {
            assert!(error.is_server_failure());
        }
    }
}

#[tokio::test]
async fn rejects_external_endpoint_paths_before_network_access() {
    let transport = transport("http://127.0.0.1:1/v1".to_owned());
    for endpoint in ["https://evil.example/steal", "../escape", ""] {
        let error = transport
            .send(Method::GET, endpoint, None, false, false)
            .await
            .expect_err("unsafe endpoint should fail");
        assert!(matches!(error, Error::InvalidConfiguration(_)));
    }
}

#[tokio::test]
async fn rejects_declared_oversized_responses() {
    let body = b"0123456789";
    let base_url = serve_response(200, "", body).await;
    let config = ClientConfig::new_insecure_http(base_url, "failure-test/1.0")
        .expect("config")
        .with_max_response_bytes(4)
        .expect("response limit");
    let (private_key, _) = common::client_keys();
    let transport = TransportBuilder::new(config)
        .build(private_key)
        .expect("transport");
    let error = transport
        .send_unverified(Method::GET, "test", None, false)
        .await
        .expect_err("oversized response should fail");
    assert!(matches!(error, Error::ResponseTooLarge { limit: 4 }));
}

#[test]
fn signs_and_verifies_a_known_failure_fixture() {
    let (private_key, public_key) = common::client_keys();
    let body = b"failure-fixture";
    let signature = sign(&private_key, body).expect("signature");
    bunq_api::verify(&public_key, body, &signature).expect("fixture signature");
}
