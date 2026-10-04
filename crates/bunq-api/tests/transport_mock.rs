use bunq_api::{ClientConfig, Method, TransportBuilder, sign, verify};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

#[path = "support/mod.rs"]
mod common;

#[tokio::test]
async fn transport_sends_signed_request_and_verifies_response() {
    let (client_private_key, client_public_key) = common::client_keys();
    let (server_private_key, server_public_key) = common::server_keys();
    let response_body = br#"{"Response":[{"UserPerson":{"id":42}}]}"#.to_vec();
    let response_signature = sign(&server_private_key, &response_body).expect("response signature");
    let listener = TcpListener::bind(("127.0.0.1", 0)).await.expect("listener");
    let address = listener.local_addr().expect("listener address");
    let expected_body = br#"{"hello":"bunq"}"#.to_vec();
    let expected_body_for_server = expected_body.clone();
    let response_body_for_server = response_body.clone();
    let expected_public_key = client_public_key.clone();
    let server = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.expect("request");
        let mut request = Vec::new();
        let body_start;
        loop {
            let mut buffer = [0_u8; 1024];
            let read = socket.read(&mut buffer).await.expect("request bytes");
            assert_ne!(read, 0);
            request.extend_from_slice(&buffer[..read]);
            if let Some(index) = request.windows(4).position(|window| window == b"\r\n\r\n") {
                body_start = index + 4;
                break;
            }
        }
        let header_text = String::from_utf8_lossy(&request[..body_start]).into_owned();
        assert!(header_text.starts_with("POST /v1/test HTTP/1.1\r\n"));
        assert!(header_text.contains("user-agent: bunq-api-test/1.0"));
        assert!(header_text.contains("cache-control: no-cache"));
        assert!(header_text.contains("x-bunq-client-authentication: session-token"));
        let signature = header_text
            .lines()
            .find_map(|line| line.strip_prefix("x-bunq-client-signature: "))
            .expect("request signature");
        let request_id = header_text
            .lines()
            .find_map(|line| line.strip_prefix("x-bunq-client-request-id: "))
            .expect("request ID");
        let content_length = header_text
            .lines()
            .find_map(|line| line.strip_prefix("content-length: "))
            .and_then(|value| value.parse::<usize>().ok())
            .expect("content length");
        while request.len() - body_start < content_length {
            let mut buffer = [0_u8; 1024];
            let read = socket.read(&mut buffer).await.expect("request body bytes");
            assert_ne!(read, 0);
            request.extend_from_slice(&buffer[..read]);
        }
        verify(
            &expected_public_key,
            &request[body_start..body_start + content_length],
            signature,
        )
        .expect("request signature verification");
        assert_eq!(
            &request[body_start..body_start + content_length],
            expected_body_for_server.as_slice()
        );
        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nX-Bunq-Client-Request-Id: {}\r\nX-Bunq-Server-Signature: {}\r\nX-Bunq-Client-Response-Id: response-1\r\n\r\n",
            response_body_for_server.len(),
            request_id,
            response_signature
        );
        socket
            .write_all(response.as_bytes())
            .await
            .expect("response headers");
        socket
            .write_all(&response_body_for_server)
            .await
            .expect("response body");
    });

    let config =
        ClientConfig::new_insecure_http(format!("http://{address}/v1"), "bunq-api-test/1.0")
            .expect("config");
    let mut transport = TransportBuilder::new(config)
        .build(client_private_key)
        .expect("transport");
    transport.set_authentication_token(Some("session-token".to_owned()));
    transport.set_server_key(Some(server_public_key));
    let response = transport
        .send(Method::POST, "test", Some(expected_body), true, true)
        .await
        .expect("transport response");
    assert_eq!(response.status, 200);
    assert_eq!(
        response.response_request_id.as_deref(),
        Some(response.request_id.as_str())
    );
    assert_eq!(response.response_id.as_deref(), Some("response-1"));
    assert_eq!(response.body, response_body);
    server.await.expect("server task");
}
