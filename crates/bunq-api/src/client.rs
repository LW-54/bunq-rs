use serde::de::DeserializeOwned;

use crate::{
    CreatedPayment, CreatedRequestInquiry, Error, InstallationContext, Method, MonetaryAccountBank,
    MonetaryAccountExternal, MonetaryAccountSavings, PaginatedResponse, Payment, PaymentBatch,
    PaymentBatchRequest, PaymentRequest, RequestInquiry, RequestInquiryRequest, Response,
    ResponseEnvelope, Result, Transport, User,
};

/// An authenticated bunq API client backed by a persisted installation.
pub struct Client {
    transport: Transport,
}

impl std::fmt::Debug for Client {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Client")
            .field("transport", &self.transport)
            .finish()
    }
}

impl Client {
    /// Creates a client by opening a fresh session for a registered installation.
    ///
    /// # Errors
    ///
    /// Returns an error if persisted credentials are invalid, session creation fails, or bunq
    /// returns an invalid session response.
    pub async fn connect(context: &InstallationContext) -> Result<(Self, crate::Session)> {
        let session = context.create_session().await?;
        let client = Self::from_installation(context, session.token())?;
        Ok((client, session))
    }

    /// Restores a client from an installation context and session token.
    ///
    /// # Errors
    ///
    /// Returns an error when persisted key material or the HTTP configuration is invalid.
    pub fn from_installation(
        context: &InstallationContext,
        session_token: impl Into<String>,
    ) -> Result<Self> {
        let session_token = session_token.into();
        if session_token.trim().is_empty() {
            return Err(Error::InvalidConfiguration(
                "session token must not be empty".to_owned(),
            ));
        }
        let mut transport = context.transport()?;
        transport.set_authentication_token(Some(session_token));
        Ok(Self { transport })
    }

    /// Replaces the current session with a newly created one.
    ///
    /// This method is explicit because bunq's `401` response can indicate either an expired token
    /// or an invalid signature; callers should decide when renewal is appropriate.
    ///
    /// # Errors
    ///
    /// Returns an error if session creation fails or the new session cannot be installed.
    pub async fn renew_session(&mut self, context: &InstallationContext) -> Result<crate::Session> {
        let session = context.create_session().await?;
        self.transport
            .set_authentication_token(Some(session.token().to_owned()));
        Ok(session)
    }

    /// Sends an authenticated request and validates the server response signature.
    ///
    /// # Errors
    ///
    /// Returns an error for request construction or network failures, signature failures, malformed
    /// bunq responses, and non-success HTTP responses.
    pub async fn request(
        &self,
        method: Method,
        endpoint: &str,
        body: Option<Vec<u8>>,
        sign_body: bool,
    ) -> Result<Response> {
        self.transport
            .send(method, endpoint, body, sign_body, true)
            .await
    }

    /// Sends an authenticated request and decodes its top-level `Response` array.
    ///
    /// # Errors
    ///
    /// Returns an error for request, signature, HTTP, JSON, or envelope validation failures.
    pub async fn request_json<T: DeserializeOwned>(
        &self,
        method: Method,
        endpoint: &str,
        body: Option<Vec<u8>>,
        sign_body: bool,
    ) -> Result<ResponseEnvelope<T>> {
        let response = self.request(method, endpoint, body, sign_body).await?;
        crate::decode_response(&response.body)
    }

    /// Retrieves the authenticated user.
    ///
    /// # Errors
    ///
    /// Returns an error for transport, signature, API, or response decoding failures.
    pub async fn get_user(&self) -> Result<ResponseEnvelope<User>> {
        self.request_json(Method::GET, "user", None, false).await
    }

    /// Lists bank accounts belonging to a user.
    ///
    /// # Errors
    ///
    /// Returns an error for transport, signature, API, or response decoding failures.
    pub async fn list_monetary_account_banks(
        &self,
        user_id: u64,
    ) -> Result<ResponseEnvelope<MonetaryAccountBank>> {
        self.request_json(
            Method::GET,
            &format!("user/{user_id}/monetary-account-bank"),
            None,
            false,
        )
        .await
    }

    /// Retrieves one bank account by ID.
    ///
    /// # Errors
    ///
    /// Returns an error for transport, signature, API, or response decoding failures.
    pub async fn get_monetary_account_bank(
        &self,
        user_id: u64,
        account_id: u64,
    ) -> Result<ResponseEnvelope<MonetaryAccountBank>> {
        self.request_json(
            Method::GET,
            &format!("user/{user_id}/monetary-account-bank/{account_id}"),
            None,
            false,
        )
        .await
    }

    /// Lists external accounts linked to a user.
    ///
    /// # Errors
    ///
    /// Returns an error for transport, signature, API, or response decoding failures.
    pub async fn list_monetary_account_externals(
        &self,
        user_id: u64,
    ) -> Result<ResponseEnvelope<MonetaryAccountExternal>> {
        self.request_json(
            Method::GET,
            &format!("user/{user_id}/monetary-account-external"),
            None,
            false,
        )
        .await
    }

    /// Retrieves one external account by ID.
    ///
    /// # Errors
    ///
    /// Returns an error for transport, signature, API, or response decoding failures.
    pub async fn get_monetary_account_external(
        &self,
        user_id: u64,
        account_id: u64,
    ) -> Result<ResponseEnvelope<MonetaryAccountExternal>> {
        self.request_json(
            Method::GET,
            &format!("user/{user_id}/monetary-account-external/{account_id}"),
            None,
            false,
        )
        .await
    }

    /// Lists savings accounts belonging to a user.
    ///
    /// # Errors
    ///
    /// Returns an error for transport, signature, API, or response decoding failures.
    pub async fn list_monetary_account_savings(
        &self,
        user_id: u64,
    ) -> Result<ResponseEnvelope<MonetaryAccountSavings>> {
        self.request_json(
            Method::GET,
            &format!("user/{user_id}/monetary-account-savings"),
            None,
            false,
        )
        .await
    }

    /// Retrieves one savings account by ID.
    ///
    /// # Errors
    ///
    /// Returns an error for transport, signature, API, or response decoding failures.
    pub async fn get_monetary_account_savings(
        &self,
        user_id: u64,
        account_id: u64,
    ) -> Result<ResponseEnvelope<MonetaryAccountSavings>> {
        self.request_json(
            Method::GET,
            &format!("user/{user_id}/monetary-account-savings/{account_id}"),
            None,
            false,
        )
        .await
    }

    /// Lists payments for a monetary account.
    ///
    /// `count` must be between 1 and bunq's documented maximum of 200. Use
    /// the returned pagination URLs for subsequent pages.
    ///
    /// # Errors
    ///
    /// Returns an error for an invalid count, transport, signature, API, or response decoding failure.
    pub async fn list_payments(
        &self,
        user_id: u64,
        account_id: u64,
        count: Option<u16>,
    ) -> Result<PaginatedResponse<Payment>> {
        if count.is_some_and(|value| !(1..=200).contains(&value)) {
            return Err(Error::InvalidConfiguration(
                "payment count must be between 1 and 200".to_owned(),
            ));
        }
        let endpoint = count.map_or_else(
            || format!("user/{user_id}/monetary-account/{account_id}/payment"),
            |count| format!("user/{user_id}/monetary-account/{account_id}/payment?count={count}"),
        );
        let response = self.request(Method::GET, &endpoint, None, false).await?;
        serde_json::from_slice(&response.body).map_err(Error::Serialization)
    }

    /// Lists request-inquiries for a monetary account.
    ///
    /// `count` must be between 1 and 200 when provided.
    ///
    /// # Errors
    ///
    /// Returns an error for an invalid count, transport, signature, API, or response decoding failure.
    pub async fn list_request_inquiries(
        &self,
        user_id: u64,
        account_id: u64,
        count: Option<u16>,
    ) -> Result<PaginatedResponse<RequestInquiry>> {
        if count.is_some_and(|value| !(1..=200).contains(&value)) {
            return Err(Error::InvalidConfiguration(
                "request-inquiry count must be between 1 and 200".to_owned(),
            ));
        }
        let endpoint = count.map_or_else(
            || format!("user/{user_id}/monetary-account/{account_id}/request-inquiry"),
            |count| {
                format!(
                    "user/{user_id}/monetary-account/{account_id}/request-inquiry?count={count}"
                )
            },
        );
        let response = self.request(Method::GET, &endpoint, None, false).await?;
        serde_json::from_slice(&response.body).map_err(Error::Serialization)
    }

    /// Retrieves one request-inquiry by ID.
    ///
    /// # Errors
    ///
    /// Returns an error for transport, signature, API, or response decoding failures.
    pub async fn get_request_inquiry(
        &self,
        user_id: u64,
        account_id: u64,
        inquiry_id: u64,
    ) -> Result<ResponseEnvelope<RequestInquiry>> {
        self.request_json(
            Method::GET,
            &format!("user/{user_id}/monetary-account/{account_id}/request-inquiry/{inquiry_id}"),
            None,
            false,
        )
        .await
    }

    /// Lists payment batches for a monetary account.
    ///
    /// `count` must be between 1 and 200 when provided.
    ///
    /// # Errors
    ///
    /// Returns an error for an invalid count, transport, signature, API, or response decoding failure.
    pub async fn list_payment_batches(
        &self,
        user_id: u64,
        account_id: u64,
        count: Option<u16>,
    ) -> Result<PaginatedResponse<PaymentBatch>> {
        if count.is_some_and(|value| !(1..=200).contains(&value)) {
            return Err(Error::InvalidConfiguration(
                "payment-batch count must be between 1 and 200".to_owned(),
            ));
        }
        let endpoint = count.map_or_else(
            || format!("user/{user_id}/monetary-account/{account_id}/payment-batch"),
            |count| {
                format!("user/{user_id}/monetary-account/{account_id}/payment-batch?count={count}")
            },
        );
        let response = self.request(Method::GET, &endpoint, None, false).await?;
        serde_json::from_slice(&response.body).map_err(Error::Serialization)
    }

    /// Retrieves one payment batch by ID.
    ///
    /// # Errors
    ///
    /// Returns an error for transport, signature, API, or response decoding failures.
    pub async fn get_payment_batch(
        &self,
        user_id: u64,
        account_id: u64,
        batch_id: u64,
    ) -> Result<ResponseEnvelope<PaymentBatch>> {
        self.request_json(
            Method::GET,
            &format!("user/{user_id}/monetary-account/{account_id}/payment-batch/{batch_id}"),
            None,
            false,
        )
        .await
    }

    /// Follows one bunq pagination URL returned by a previous list response.
    ///
    /// The transport accepts only relative URLs under the configured API base URL, so callers
    /// cannot accidentally redirect a request to another host through this helper.
    ///
    /// # Errors
    ///
    /// Returns an error for an invalid pagination URL, transport, signature, API, or response
    /// decoding failure.
    pub async fn list_payments_page(
        &self,
        pagination_url: &str,
    ) -> Result<PaginatedResponse<Payment>> {
        let response = self
            .request(Method::GET, pagination_url, None, false)
            .await?;
        serde_json::from_slice(&response.body).map_err(Error::Serialization)
    }

    /// Collects payment history by following older pages up to a caller-provided page limit.
    ///
    /// This helper performs no retries and stops when bunq provides no older page or when
    /// `max_pages` is reached. A zero page limit returns an empty vector without a request.
    ///
    /// # Errors
    ///
    /// Returns an error for invalid count values, pagination URLs, transport, signature, API,
    /// or response decoding failures.
    pub async fn list_all_payments(
        &self,
        user_id: u64,
        account_id: u64,
        count: Option<u16>,
        max_pages: usize,
    ) -> Result<Vec<Payment>> {
        if max_pages == 0 {
            return Ok(Vec::new());
        }
        let mut payments = Vec::new();
        let mut page = self.list_payments(user_id, account_id, count).await?;
        payments.append(&mut page.response);
        for _ in 1..max_pages {
            let Some(next_url) = page.pagination.older_url.as_deref() else {
                break;
            };
            page = self.list_payments_page(next_url).await?;
            payments.append(&mut page.response);
        }
        Ok(payments)
    }

    /// Retrieves one payment by ID.
    ///
    /// # Errors
    ///
    /// Returns an error for transport, signature, API, or response decoding failures.
    pub async fn get_payment(
        &self,
        user_id: u64,
        account_id: u64,
        payment_id: u64,
    ) -> Result<ResponseEnvelope<Payment>> {
        self.request_json(
            Method::GET,
            &format!("user/{user_id}/monetary-account/{account_id}/payment/{payment_id}"),
            None,
            false,
        )
        .await
    }

    /// Creates one payment using a signed request.
    ///
    /// This method performs one network attempt and never automatically retries.
    /// If the network outcome is uncertain, inspect the account's payments before
    /// deciding whether a recovery action is safe.
    ///
    /// # Errors
    ///
    /// Returns an error for request serialization, transport, signature, API, or response decoding failures.
    pub async fn create_payment(
        &self,
        user_id: u64,
        account_id: u64,
        request: &PaymentRequest,
    ) -> Result<ResponseEnvelope<CreatedPayment>> {
        let body = serde_json::to_vec(request)?;
        self.request_json(
            Method::POST,
            &format!("user/{user_id}/monetary-account/{account_id}/payment"),
            Some(body),
            true,
        )
        .await
    }

    /// Creates a signed batch of outgoing payments.
    ///
    /// This operation makes one network attempt and is not automatically retried.
    ///
    /// # Errors
    ///
    /// Returns an error for batch serialization, transport, signature, API, or response decoding failures.
    pub async fn create_payment_batch(
        &self,
        user_id: u64,
        account_id: u64,
        batch: &PaymentBatchRequest,
    ) -> Result<ResponseEnvelope<CreatedPayment>> {
        let body = serde_json::to_vec(batch)?;
        self.request_json(
            Method::POST,
            &format!("user/{user_id}/monetary-account/{account_id}/payment-batch"),
            Some(body),
            true,
        )
        .await
    }

    /// Creates a signed request for money from a counterparty.
    ///
    /// This operation makes one network attempt and is not automatically retried.
    ///
    /// # Errors
    ///
    /// Returns an error for request serialization, transport, signature, API, or response decoding failures.
    pub async fn create_request_inquiry(
        &self,
        user_id: u64,
        account_id: u64,
        request: &RequestInquiryRequest,
    ) -> Result<ResponseEnvelope<CreatedRequestInquiry>> {
        let body = serde_json::to_vec(request)?;
        self.request_json(
            Method::POST,
            &format!("user/{user_id}/monetary-account/{account_id}/request-inquiry"),
            Some(body),
            true,
        )
        .await
    }
}
