use bunq_api::{SandboxUserKind, create_sandbox_user, install_device};

#[tokio::test]
#[allow(clippy::too_many_lines)]
async fn sandbox_person_can_create_an_authenticated_context() {
    if std::env::var("BUNQ_API_RUN_SANDBOX_TESTS").as_deref() != Ok("1") {
        return;
    }

    let api_key = create_sandbox_user(SandboxUserKind::Person)
        .await
        .expect("sandbox user creation should succeed");
    let (context, session) = install_device(
        api_key,
        "https://public-api.sandbox.bunq.com/v1",
        "bunq-api-sandbox-test/0.1",
        "bunq-api integration test",
    )
    .await
    .expect("sandbox installation should succeed");

    assert!(context.registered_device_id() > 0);
    assert_ne!(session.token(), "");

    let client = bunq_api::Client::from_installation(&context, session.token())
        .expect("client should restore from sandbox context");
    let (reconnected_client, reconnected_session) = bunq_api::Client::connect(&context)
        .await
        .expect("client should reconnect from sandbox context");
    assert_eq!(reconnected_session.user_id(), session.user_id());
    let mut renewable_client = reconnected_client;
    let renewed_session = renewable_client
        .renew_session(&context)
        .await
        .expect("client should renew its session");
    assert_eq!(renewed_session.user_id(), session.user_id());
    let user = client.get_user().await.expect("user lookup should succeed");
    assert_ne!(user.response.len(), 0);
    let accounts = client
        .list_monetary_account_banks(session.user_id())
        .await
        .expect("account lookup should succeed");
    assert_ne!(accounts.response.len(), 0);
    let account_id = accounts.response[0].id;
    let account = client
        .get_monetary_account_bank(session.user_id(), account_id)
        .await
        .expect("account detail lookup should succeed");
    assert_eq!(account.response[0].id, account_id);
    let mut bank_request =
        bunq_api::MonetaryAccountBankRequest::new("EUR").expect("bank currency should validate");
    bank_request.description = Some("bunq-api sandbox bank creation".to_owned());
    bank_request.display_name = Some("bunq-api sandbox test".to_owned());
    let created_bank = client
        .create_monetary_account_bank(session.user_id(), &bank_request)
        .await
        .expect("sandbox bank account creation should succeed");
    assert_eq!(created_bank.response.len(), 1);
    assert!(created_bank.response[0].id > 0);
    let created_bank_detail = client
        .get_monetary_account_bank(session.user_id(), created_bank.response[0].id)
        .await
        .expect("created bank account lookup should succeed");
    assert_eq!(
        created_bank_detail.response[0].id,
        created_bank.response[0].id
    );

    let mut savings_request = bunq_api::MonetaryAccountSavingsRequest::new("EUR")
        .expect("savings currency should validate");
    savings_request.description = Some("bunq-api sandbox savings creation".to_owned());
    let created_savings = client
        .create_monetary_account_savings(session.user_id(), &savings_request)
        .await
        .expect("sandbox savings account creation should succeed");
    assert_eq!(created_savings.response.len(), 1);
    assert!(created_savings.response[0].id > 0);
    let created_savings_detail = client
        .get_monetary_account_savings(session.user_id(), created_savings.response[0].id)
        .await
        .expect("created savings account lookup should succeed");
    assert_eq!(
        created_savings_detail.response[0].id,
        created_savings.response[0].id
    );

    let funding_request = bunq_api::RequestInquiryRequest::new(
        "1.00",
        "EUR",
        bunq_api::CounterpartyAlias::Email,
        "sugardaddy@bunq.com",
        Some("bunq-api sandbox test funding".to_owned()),
    )
    .expect("funding request should validate");
    let funding = client
        .create_request_inquiry(session.user_id(), account_id, &funding_request)
        .await
        .expect("sandbox funding request should succeed");
    assert!(funding.response[0].id > 0);
    let mut funded = false;
    for _ in 0..10 {
        let refreshed_accounts = client
            .list_monetary_account_banks(session.user_id())
            .await
            .expect("account refresh should succeed");
        if refreshed_accounts.response.iter().any(|account| {
            account
                .balance
                .as_ref()
                .and_then(|balance| balance.value.parse::<f64>().ok())
                .is_some_and(|value| value >= 1.00)
        }) {
            funded = true;
            break;
        }
        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
    }
    assert!(funded, "sandbox funding did not arrive before timeout");
    let payments = client
        .list_payments(session.user_id(), account_id, Some(10))
        .await
        .expect("payment lookup should succeed");
    assert!(payments.response.len() <= 10);
    if let Some(next_url) = payments.pagination.older_url.as_deref() {
        let next_page = client
            .list_payments_page(next_url)
            .await
            .expect("pagination URL should be followable");
        assert!(next_page.response.len() <= 200);
    }
    let payment_request = bunq_api::PaymentRequest::new(
        "0.01",
        "EUR",
        bunq_api::CounterpartyAlias::Email,
        "sugardaddy@bunq.com",
        Some("bunq-api sandbox signed payment test".to_owned()),
    )
    .expect("payment request should validate");
    let created = client
        .create_payment(session.user_id(), account_id, &payment_request)
        .await
        .expect("sandbox payment creation should succeed");
    assert_ne!(created.response.len(), 0);
    assert!(created.response[0].id > 0);
    let payment = client
        .get_payment(session.user_id(), account_id, created.response[0].id)
        .await
        .expect("payment detail lookup should succeed");
    assert_eq!(payment.response[0].id, created.response[0].id);
    let batch_payment = bunq_api::PaymentRequest::new(
        "0.01",
        "EUR",
        bunq_api::CounterpartyAlias::Email,
        "sugardaddy@bunq.com",
        Some("bunq-api sandbox batch test".to_owned()),
    )
    .expect("batch payment should validate");
    let batch = bunq_api::PaymentBatchRequest::new(vec![batch_payment])
        .expect("payment batch should validate");
    let created_batch = client
        .create_payment_batch(session.user_id(), account_id, &batch)
        .await
        .expect("sandbox payment batch should succeed");
    assert_ne!(created_batch.response.len(), 0);
}
