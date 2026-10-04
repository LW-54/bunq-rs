use bunq_api::{ClientConfig, Error, Method, TransportBuilder, sign};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

#[path = "support/mod.rs"]
mod common;

#[tokio::test]
async fn rejects_mismatched_response_request_id_after_valid_signature() {
    let (client_private_key, _) = common::client_keys();
    let (server_private_key, server_public_key) = common::server_keys();
    let response_body = br#"{"Response":[{"Id":{"id":1}}]}"#;
    let response_signature = sign(&server_private_key, response_body).expect("response signature");
    let listener = TcpListener::bind(("127.0.0.1", 0)).await.expect("listener");
    let address = listener.local_addr().expect("address");
    let server = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.expect("request");
        let mut request = [0_u8; 2048];
        let read = socket.read(&mut request).await.expect("request bytes");
        assert_ne!(read, 0);
        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nX-Bunq-Client-Request-Id: wrong-id\r\nX-Bunq-Server-Signature: {}\r\n\r\n",
            response_body.len(),
            response_signature
        );
        socket
            .write_all(response.as_bytes())
            .await
            .expect("headers");
        socket.write_all(response_body).await.expect("body");
    });
    let config =
        ClientConfig::new_insecure_http(format!("http://{address}/v1"), "correlation-test/1.0")
            .expect("config");
    let mut transport = TransportBuilder::new(config)
        .build(client_private_key)
        .expect("transport");
    transport.set_server_key(Some(server_public_key));
    let error = transport
        .send(Method::GET, "test", None, false, true)
        .await
        .expect_err("correlation mismatch should fail");
    assert!(matches!(error, Error::ResponseRequestIdMismatch { .. }));
    server.await.expect("server task");
}
