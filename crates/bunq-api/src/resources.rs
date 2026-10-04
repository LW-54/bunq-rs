use std::collections::HashMap;

use serde::{Deserialize, Deserializer, Serialize, Serializer, de::Error as DeError};
use serde_json::Value;

/// A bunq monetary balance.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Balance {
    pub currency: String,
    pub value: String,
}

/// An exact monetary amount represented in minor units.
///
/// `minor_units` is signed because response amounts can represent incoming or
/// outgoing transactions. Request constructors reject non-positive amounts
/// where the API operation requires a positive amount.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Money {
    pub currency: String,
    pub minor_units: i64,
    pub scale: u8,
}

impl Money {
    /// Creates an exact amount from minor units and an explicit decimal scale.
    ///
    /// # Errors
    ///
    /// Returns an error when the currency is not a three-letter uppercase code.
    pub fn from_minor_units(
        currency: impl Into<String>,
        minor_units: i64,
        scale: u8,
    ) -> crate::Result<Self> {
        let currency = currency.into();
        validate_currency(&currency)?;
        Ok(Self {
            currency,
            minor_units,
            scale,
        })
    }

    /// Creates a positive amount from a decimal string without using floating point.
    ///
    /// # Errors
    ///
    /// Returns an error for malformed, non-positive, or over-precise decimal values.
    pub fn from_decimal(
        currency: impl Into<String>,
        value: &str,
        scale: u8,
    ) -> crate::Result<Self> {
        let currency = currency.into();
        validate_currency(&currency)?;
        let minor_units = parse_minor_units(value, scale)?;
        if minor_units <= 0 {
            return Err(crate::Error::InvalidConfiguration(
                "amount must be positive".to_owned(),
            ));
        }
        Ok(Self {
            currency,
            minor_units,
            scale,
        })
    }

    fn to_wire_value(&self) -> String {
        let negative = self.minor_units < 0;
        let magnitude = self.minor_units.unsigned_abs();
        let scale = usize::from(self.scale);
        let digits = magnitude.to_string();
        let (whole, fraction) = if scale == 0 {
            (digits, String::new())
        } else if digits.len() <= scale {
            ("0".to_owned(), format!("{digits:0>scale$}"))
        } else {
            let split = digits.len().saturating_sub(scale);
            (
                digits.chars().take(split).collect(),
                digits.chars().skip(split).collect(),
            )
        };
        let value = if scale == 0 {
            whole
        } else {
            format!("{whole}.{fraction}")
        };
        if negative { format!("-{value}") } else { value }
    }
}

impl Serialize for Money {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        Balance {
            currency: self.currency.clone(),
            value: self.to_wire_value(),
        }
        .serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for Money {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let balance = Balance::deserialize(deserializer)?;
        let minor_units = parse_minor_units_signed(&balance.value, 2).map_err(D::Error::custom)?;
        Ok(Self {
            currency: balance.currency,
            minor_units,
            scale: 2,
        })
    }
}

fn validate_currency(currency: &str) -> crate::Result<()> {
    if currency.len() != 3 || !currency.bytes().all(|byte| byte.is_ascii_uppercase()) {
        return Err(crate::Error::InvalidConfiguration(
            "currency must be a three-letter uppercase code".to_owned(),
        ));
    }
    Ok(())
}

fn parse_minor_units(value: &str, scale: u8) -> crate::Result<i64> {
    let parsed = parse_minor_units_signed(value, scale)?;
    if parsed < 0 {
        return Err(crate::Error::InvalidConfiguration(
            "amount must not be negative".to_owned(),
        ));
    }
    Ok(parsed)
}

fn parse_minor_units_signed(value: &str, scale: u8) -> crate::Result<i64> {
    let negative = value.starts_with('-');
    let unsigned = value.strip_prefix('-').unwrap_or(value);
    let (whole, fraction) = unsigned.split_once('.').unwrap_or((unsigned, ""));
    if whole.is_empty()
        || !whole.bytes().all(|byte| byte.is_ascii_digit())
        || !fraction.bytes().all(|byte| byte.is_ascii_digit())
        || fraction.len() > usize::from(scale)
    {
        return Err(crate::Error::InvalidConfiguration(
            "invalid decimal amount".to_owned(),
        ));
    }
    let mut digits = whole.to_owned();
    digits.push_str(fraction);
    digits.extend(std::iter::repeat_n(
        '0',
        usize::from(scale).saturating_sub(fraction.len()),
    ));
    let parsed = digits
        .parse::<i64>()
        .map_err(|_| crate::Error::InvalidConfiguration("amount is too large".to_owned()))?;
    if negative {
        parsed
            .checked_neg()
            .ok_or_else(|| crate::Error::InvalidConfiguration("amount is too large".to_owned()))
    } else {
        Ok(parsed)
    }
}

/// A person user response item.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
pub struct UserPerson {
    pub id: u64,
    #[serde(default)]
    pub display_name: Option<String>,
    #[serde(flatten)]
    pub additional: HashMap<String, Value>,
}

/// A company user response item.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
pub struct UserCompany {
    pub id: u64,
    #[serde(default)]
    pub display_name: Option<String>,
    #[serde(flatten)]
    pub additional: HashMap<String, Value>,
}

/// A user response item. bunq returns either `UserPerson` or `UserCompany`.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
pub struct User {
    #[serde(rename = "UserPerson", default)]
    pub person: Option<UserPerson>,
    #[serde(rename = "UserCompany", default)]
    pub company: Option<UserCompany>,
}

/// A bank monetary-account response item.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MonetaryAccountBank {
    pub id: u64,
    pub balance: Option<Balance>,
    pub description: Option<String>,
    pub status: Option<String>,
    pub additional: HashMap<String, Value>,
}

/// An external bank account linked to the bunq user.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MonetaryAccountExternal {
    pub id: u64,
    pub balance: Option<Balance>,
    pub description: Option<String>,
    pub status: Option<String>,
    pub additional: HashMap<String, Value>,
}

/// A bunq savings account.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MonetaryAccountSavings {
    pub id: u64,
    pub balance: Option<Balance>,
    pub description: Option<String>,
    pub status: Option<String>,
    pub additional: HashMap<String, Value>,
}

macro_rules! impl_monetary_account_deserialize {
    ($type:ty, $wrapper:literal) => {
        impl<'de> Deserialize<'de> for $type {
            fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
            where
                D: Deserializer<'de>,
            {
                let value = Value::deserialize(deserializer)?;
                let account = value.get($wrapper).unwrap_or(&value).clone();
                let fields = serde_json::from_value::<MonetaryAccountFields>(account)
                    .map_err(D::Error::custom)?;
                Ok(Self {
                    id: fields.id,
                    balance: fields.balance,
                    description: fields.description,
                    status: fields.status,
                    additional: fields.additional,
                })
            }
        }
    };
}

impl_monetary_account_deserialize!(MonetaryAccountExternal, "MonetaryAccountExternal");
impl_monetary_account_deserialize!(MonetaryAccountSavings, "MonetaryAccountSavings");

/// Navigation URLs returned for a paginated bunq list response.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
pub struct Pagination {
    #[serde(default)]
    pub future_url: Option<String>,
    #[serde(default)]
    pub newer_url: Option<String>,
    #[serde(default)]
    pub older_url: Option<String>,
}

/// A bunq list response with navigation metadata.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
pub struct PaginatedResponse<T> {
    #[serde(rename = "Response")]
    pub response: Vec<T>,
    #[serde(rename = "Pagination")]
    pub pagination: Pagination,
}

/// A payment response item.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Payment {
    pub id: u64,
    pub amount: Option<Balance>,
    pub description: Option<String>,
    pub created: Option<String>,
    pub updated: Option<String>,
    pub additional: HashMap<String, Value>,
}

/// The result returned after creating a payment.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CreatedPayment {
    pub id: u64,
    pub payment: Option<Payment>,
}

impl<'de> Deserialize<'de> for CreatedPayment {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = Value::deserialize(deserializer)?;
        if let Some(id) = value
            .get("Id")
            .and_then(|entry| entry.get("id"))
            .and_then(Value::as_u64)
        {
            return Ok(Self { id, payment: None });
        }
        if let Some(payment_value) = value.get("Payment") {
            let payment = serde_json::from_value::<Payment>(payment_value.clone())
                .map_err(D::Error::custom)?;
            return Ok(Self {
                id: payment.id,
                payment: Some(payment),
            });
        }
        Err(D::Error::custom(
            "payment creation response is missing Id or Payment",
        ))
    }
}

/// The supported alias types for a payment counterparty.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum CounterpartyAlias {
    Email,
    PhoneNumber,
    Iban,
}

/// A normalized and checksum-validated International Bank Account Number.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct Iban(String);

impl Iban {
    /// Parses and normalizes an IBAN by removing spaces and uppercasing letters.
    ///
    /// # Errors
    ///
    /// Returns an error when the IBAN has an invalid length, characters, country prefix, or MOD-97 checksum.
    pub fn parse(value: impl AsRef<str>) -> crate::Result<Self> {
        let normalized: String = value
            .as_ref()
            .chars()
            .filter(|character| !character.is_ascii_whitespace())
            .flat_map(char::to_uppercase)
            .collect();
        if !(15..=34).contains(&normalized.len())
            || !normalized
                .chars()
                .take(2)
                .all(|character| character.is_ascii_alphabetic())
            || !normalized
                .chars()
                .skip(2)
                .all(|character| character.is_ascii_alphanumeric())
        {
            return Err(crate::Error::InvalidConfiguration(
                "invalid IBAN format".to_owned(),
            ));
        }
        let characters: Vec<char> = normalized.chars().collect();
        let mut remainder = 0_u32;
        for character in characters.iter().skip(4).chain(characters.iter().take(4)) {
            let value = character.to_digit(36).ok_or_else(|| {
                crate::Error::InvalidConfiguration("invalid IBAN format".to_owned())
            })?;
            let digits = value.to_string();
            for digit in digits.chars() {
                let digit = digit.to_digit(10).unwrap_or(0);
                remainder = remainder
                    .checked_mul(10)
                    .and_then(|value| value.checked_add(digit))
                    .ok_or_else(|| {
                        crate::Error::InvalidConfiguration("invalid IBAN value".to_owned())
                    })?
                    % 97;
            }
        }
        if remainder != 1 {
            return Err(crate::Error::InvalidConfiguration(
                "invalid IBAN checksum".to_owned(),
            ));
        }
        Ok(Self(normalized))
    }

    /// Returns the normalized IBAN string used on the wire.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for Iban {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl Serialize for Iban {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.0)
    }
}

/// A payment counterparty identified by an email, phone number, or IBAN.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct Counterparty {
    #[serde(rename = "type")]
    pub kind: CounterpartyAlias,
    pub value: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}

/// A validated payment creation request.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct PaymentRequest {
    pub amount: Money,
    pub counterparty_alias: Counterparty,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

/// A bounded batch of outgoing payments.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct PaymentBatchRequest {
    pub payments: Vec<PaymentRequest>,
}

impl PaymentBatchRequest {
    /// Builds a payment batch. Bunq documents a maximum of 350 payments per batch.
    ///
    /// # Errors
    ///
    /// Returns an error when the batch is empty or exceeds 350 payments.
    pub fn new(payments: Vec<PaymentRequest>) -> crate::Result<Self> {
        if payments.is_empty() {
            return Err(crate::Error::InvalidConfiguration(
                "payment batch must contain at least one payment".to_owned(),
            ));
        }
        if payments.len() > 350 {
            return Err(crate::Error::InvalidConfiguration(
                "payment batch cannot contain more than 350 payments".to_owned(),
            ));
        }
        Ok(Self { payments })
    }
}

impl PaymentRequest {
    /// Creates a payment request after validating the amount and counterparty.
    ///
    /// # Errors
    ///
    /// Returns an error when the amount, currency, alias value, or description is invalid.
    pub fn new(
        amount_value: impl Into<String>,
        currency: impl Into<String>,
        alias_kind: CounterpartyAlias,
        alias_value: impl Into<String>,
        description: Option<String>,
    ) -> crate::Result<Self> {
        let amount_value = amount_value.into();
        let currency = currency.into();
        let alias_value = alias_value.into();
        let amount = Money::from_decimal(currency, &amount_value, 2)?;
        if alias_value.trim().is_empty() {
            return Err(crate::Error::InvalidConfiguration(
                "payment counterparty value must not be empty".to_owned(),
            ));
        }
        if description
            .as_ref()
            .is_some_and(|value| value.trim().is_empty())
        {
            return Err(crate::Error::InvalidConfiguration(
                "payment description must not be empty".to_owned(),
            ));
        }
        Ok(Self {
            amount,
            counterparty_alias: Counterparty {
                kind: alias_kind,
                value: alias_value,
                name: None,
            },
            description,
        })
    }

    /// Adds an optional human-readable counterparty name.
    #[must_use]
    pub fn with_counterparty_name(mut self, name: impl Into<String>) -> Self {
        self.counterparty_alias.name = Some(name.into());
        self
    }

    /// Creates a payment request addressed to a validated IBAN.
    ///
    /// # Errors
    ///
    /// Returns an error when the amount or currency is invalid.
    pub fn new_iban(
        amount_value: impl Into<String>,
        currency: impl Into<String>,
        iban: &Iban,
        description: Option<String>,
    ) -> crate::Result<Self> {
        Self::new(
            amount_value,
            currency,
            CounterpartyAlias::Iban,
            iban.as_str(),
            description,
        )
    }
}

/// A validated request for money from a counterparty.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct RequestInquiryRequest {
    pub amount_inquired: Money,
    pub counterparty_alias: Counterparty,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub allow_bunqme: bool,
}

impl RequestInquiryRequest {
    /// Creates a request-inquiry after validating amount and counterparty fields.
    ///
    /// # Errors
    ///
    /// Returns an error when the amount, currency, alias value, or description is invalid.
    pub fn new(
        amount_value: impl Into<String>,
        currency: impl Into<String>,
        alias_kind: CounterpartyAlias,
        alias_value: impl Into<String>,
        description: Option<String>,
    ) -> crate::Result<Self> {
        let payment =
            PaymentRequest::new(amount_value, currency, alias_kind, alias_value, description)?;
        Ok(Self {
            amount_inquired: payment.amount,
            counterparty_alias: payment.counterparty_alias,
            description: payment.description,
            allow_bunqme: false,
        })
    }

    /// Enables bunq.me completion when supported by the request.
    #[must_use]
    pub const fn allow_bunqme(mut self, allow: bool) -> Self {
        self.allow_bunqme = allow;
        self
    }

    /// Creates a money request addressed to a validated IBAN.
    ///
    /// # Errors
    ///
    /// Returns an error when the amount or currency is invalid.
    pub fn new_iban(
        amount_value: impl Into<String>,
        currency: impl Into<String>,
        iban: &Iban,
        description: Option<String>,
    ) -> crate::Result<Self> {
        Self::new(
            amount_value,
            currency,
            CounterpartyAlias::Iban,
            iban.as_str(),
            description,
        )
    }
}

/// The ID returned after creating a request-inquiry.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CreatedRequestInquiry {
    pub id: u64,
}

/// A request for money from a counterparty.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RequestInquiry {
    pub id: u64,
    pub amount_inquired: Option<Balance>,
    pub amount_responded: Option<Balance>,
    pub status: Option<String>,
    pub description: Option<String>,
    pub additional: HashMap<String, Value>,
}

/// A batch payment response item.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PaymentBatch {
    pub id: u64,
    pub status: Option<String>,
    pub description: Option<String>,
    pub additional: HashMap<String, Value>,
}

macro_rules! impl_request_resource_deserialize {
    ($type:ty, $wrapper:literal, $fields:ty) => {
        impl<'de> Deserialize<'de> for $type {
            fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
            where
                D: Deserializer<'de>,
            {
                let value = Value::deserialize(deserializer)?;
                let object = value.get($wrapper).unwrap_or(&value).clone();
                serde_json::from_value::<$fields>(object)
                    .map(Into::into)
                    .map_err(D::Error::custom)
            }
        }
    };
}

#[derive(Deserialize)]
struct RequestInquiryFields {
    id: u64,
    #[serde(default)]
    amount_inquired: Option<Balance>,
    #[serde(default)]
    amount_responded: Option<Balance>,
    #[serde(default)]
    status: Option<String>,
    #[serde(default)]
    description: Option<String>,
    #[serde(flatten)]
    additional: HashMap<String, Value>,
}

impl From<RequestInquiryFields> for RequestInquiry {
    fn from(fields: RequestInquiryFields) -> Self {
        Self {
            id: fields.id,
            amount_inquired: fields.amount_inquired,
            amount_responded: fields.amount_responded,
            status: fields.status,
            description: fields.description,
            additional: fields.additional,
        }
    }
}

#[derive(Deserialize)]
struct PaymentBatchFields {
    id: u64,
    #[serde(default)]
    status: Option<String>,
    #[serde(default)]
    description: Option<String>,
    #[serde(flatten)]
    additional: HashMap<String, Value>,
}

impl From<PaymentBatchFields> for PaymentBatch {
    fn from(fields: PaymentBatchFields) -> Self {
        Self {
            id: fields.id,
            status: fields.status,
            description: fields.description,
            additional: fields.additional,
        }
    }
}

impl_request_resource_deserialize!(RequestInquiry, "RequestInquiry", RequestInquiryFields);
impl_request_resource_deserialize!(PaymentBatch, "PaymentBatch", PaymentBatchFields);

impl<'de> Deserialize<'de> for CreatedRequestInquiry {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = Value::deserialize(deserializer)?;
        let id = value
            .get("Id")
            .and_then(|entry| entry.get("id"))
            .and_then(Value::as_u64)
            .ok_or_else(|| D::Error::custom("request-inquiry response is missing Id.id"))?;
        Ok(Self { id })
    }
}

#[derive(Deserialize)]
struct PaymentFields {
    id: u64,
    #[serde(default)]
    amount: Option<Balance>,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    created: Option<String>,
    #[serde(default)]
    updated: Option<String>,
    #[serde(flatten)]
    additional: HashMap<String, Value>,
}

impl<'de> Deserialize<'de> for Payment {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = Value::deserialize(deserializer)?;
        let payment = value.get("Payment").unwrap_or(&value).clone();
        let fields = serde_json::from_value::<PaymentFields>(payment).map_err(D::Error::custom)?;
        Ok(Self {
            id: fields.id,
            amount: fields.amount,
            description: fields.description,
            created: fields.created,
            updated: fields.updated,
            additional: fields.additional,
        })
    }
}

#[derive(Deserialize)]
struct MonetaryAccountBankFields {
    id: u64,
    #[serde(default)]
    balance: Option<Balance>,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    status: Option<String>,
    #[serde(flatten)]
    additional: HashMap<String, Value>,
}

#[derive(Deserialize)]
struct MonetaryAccountFields {
    id: u64,
    #[serde(default)]
    balance: Option<Balance>,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    status: Option<String>,
    #[serde(flatten)]
    additional: HashMap<String, Value>,
}

impl<'de> Deserialize<'de> for MonetaryAccountBank {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = Value::deserialize(deserializer)?;
        let account = value.get("MonetaryAccountBank").unwrap_or(&value).clone();
        let fields = serde_json::from_value::<MonetaryAccountBankFields>(account)
            .map_err(D::Error::custom)?;
        Ok(Self {
            id: fields.id,
            balance: fields.balance,
            description: fields.description,
            status: fields.status,
            additional: fields.additional,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::{
        CounterpartyAlias, CreatedPayment, CreatedRequestInquiry, Iban, MonetaryAccountBank,
        MonetaryAccountExternal, MonetaryAccountSavings, PaginatedResponse, Payment, PaymentBatch,
        PaymentBatchRequest, PaymentRequest, RequestInquiry, RequestInquiryRequest, User,
    };
    use crate::decode_response;

    #[test]
    fn decodes_person_user() {
        let envelope = decode_response::<User>(
            br#"{"Response":[{"UserPerson":{"id":7,"display_name":"Test User"}}]}"#,
        )
        .expect("user");
        let person = envelope.response[0].person.as_ref().expect("person");
        assert_eq!(person.id, 7);
        assert_eq!(person.display_name.as_deref(), Some("Test User"));
    }

    #[test]
    fn decodes_bank_account_balance() {
        let envelope = decode_response::<MonetaryAccountBank>(
            br#"{"Response":[{"MonetaryAccountBank":{"id":8,"balance":{"currency":"EUR","value":"0.00"}}}]}"#,
        )
        .expect("account");
        let account = &envelope.response[0];
        assert_eq!(account.id, 8);
        assert_eq!(account.balance.as_ref().expect("balance").value, "0.00");
    }

    #[test]
    fn decodes_external_and_savings_accounts() {
        let external = decode_response::<MonetaryAccountExternal>(
            br#"{"Response":[{"MonetaryAccountExternal":{"id":10,"balance":{"currency":"EUR","value":"3.00"}}}]}"#,
        )
        .expect("external account");
        assert_eq!(external.response[0].id, 10);
        let savings = decode_response::<MonetaryAccountSavings>(
            br#"{"Response":[{"MonetaryAccountSavings":{"id":11,"status":"ACTIVE"}}]}"#,
        )
        .expect("savings account");
        assert_eq!(savings.response[0].id, 11);
        assert_eq!(savings.response[0].status.as_deref(), Some("ACTIVE"));
    }

    #[test]
    fn decodes_paginated_payments_and_optional_fields() {
        let page = serde_json::from_slice::<PaginatedResponse<Payment>>(
            br#"{
                "Response":[{"Payment":{"id":9,"amount":{"currency":"EUR","value":"-0.10"},"description":"Coffee"}}],
                "Pagination":{"future_url":null,"newer_url":null,"older_url":"/v1/payment?older_id=9"}
            }"#,
        )
        .expect("payment page");
        assert_eq!(page.response[0].id, 9);
        assert_eq!(page.response[0].description.as_deref(), Some("Coffee"));
        assert_eq!(
            page.pagination.older_url.as_deref(),
            Some("/v1/payment?older_id=9")
        );
    }

    #[test]
    fn accepts_empty_paginated_payment_responses() {
        let page = serde_json::from_str::<PaginatedResponse<Payment>>(
            r#"{"Response":[],"Pagination":{"future_url":null,"newer_url":null,"older_url":null}}"#,
        )
        .expect("empty payment page");
        assert_eq!(page.response.len(), 0);
    }

    #[test]
    fn validates_and_serializes_payment_request() {
        let request = PaymentRequest::new(
            "0.10",
            "EUR",
            CounterpartyAlias::Email,
            "payee@example.com",
            Some("Test payment".to_owned()),
        )
        .expect("payment request");
        let json = serde_json::to_value(request).expect("payment JSON");
        assert_eq!(json["amount"]["value"], "0.10");
        assert_eq!(json["counterparty_alias"]["type"], "EMAIL");
    }

    #[test]
    fn parses_and_normalizes_valid_iban() {
        let iban = Iban::parse("gb82 west 1234 5698 7654 32").expect("IBAN");
        assert_eq!(iban.as_str(), "GB82WEST12345698765432");
        let request = PaymentRequest::new_iban("1.00", "EUR", &iban, None).expect("payment");
        assert_eq!(request.counterparty_alias.kind, CounterpartyAlias::Iban);
        assert_eq!(request.counterparty_alias.value, "GB82WEST12345698765432");
    }

    #[test]
    fn rejects_invalid_iban_checksum() {
        assert!(Iban::parse("GB82WEST12345698765431").is_err());
        assert!(Iban::parse("not-an-iban").is_err());
    }

    #[test]
    fn rejects_invalid_payment_request_values() {
        assert!(PaymentRequest::new("0", "EUR", CounterpartyAlias::Iban, "NL00", None).is_err());
        assert!(PaymentRequest::new("1.00", "eur", CounterpartyAlias::Iban, "NL00", None).is_err());
        assert!(PaymentRequest::new("1.00", "EUR", CounterpartyAlias::Iban, "", None).is_err());
    }

    #[test]
    fn accepts_id_only_payment_creation_response() {
        let created = serde_json::from_str::<CreatedPayment>(r#"{"Id":{"id":123}}"#)
            .expect("created payment");
        assert_eq!(created.id, 123);
        assert!(created.payment.is_none());
    }

    #[test]
    fn represents_money_as_exact_minor_units() {
        let money = super::Money::from_decimal("EUR", "12.34", 2).expect("money");
        assert_eq!(money.minor_units, 1234);
        assert_eq!(
            serde_json::to_value(money).expect("money JSON")["value"],
            "12.34"
        );
        assert!(super::Money::from_decimal("EUR", "12.345", 2).is_err());
    }

    #[test]
    fn deserializes_negative_response_money() {
        let money = serde_json::from_str::<super::Money>(r#"{"currency":"EUR","value":"-12.34"}"#)
            .expect("negative money");
        assert_eq!(money.minor_units, -1234);
        assert_eq!(
            serde_json::to_value(money).expect("money JSON")["value"],
            "-12.34"
        );
    }

    #[test]
    fn validates_and_serializes_payment_batch() {
        let payment = PaymentRequest::new(
            "0.10",
            "EUR",
            CounterpartyAlias::Email,
            "payee@example.com",
            None,
        )
        .expect("payment");
        let batch = PaymentBatchRequest::new(vec![payment]).expect("batch");
        let json = serde_json::to_value(batch).expect("batch JSON");
        assert_eq!(json["payments"].as_array().expect("payments").len(), 1);
    }

    #[test]
    fn validates_and_serializes_request_inquiry() {
        let request = RequestInquiryRequest::new(
            "1.00",
            "EUR",
            CounterpartyAlias::Email,
            "payee@example.com",
            Some("Invoice".to_owned()),
        )
        .expect("request inquiry");
        let json = serde_json::to_value(request).expect("request inquiry JSON");
        assert_eq!(json["amount_inquired"]["value"], "1.00");
        assert_eq!(json["allow_bunqme"], false);
    }

    #[test]
    fn parses_request_inquiry_creation_id() {
        let created = serde_json::from_str::<CreatedRequestInquiry>(r#"{"Id":{"id":77}}"#)
            .expect("request inquiry ID");
        assert_eq!(created.id, 77);
    }

    #[test]
    fn decodes_request_inquiry_and_payment_batch_items() {
        let inquiry = decode_response::<RequestInquiry>(
            br#"{"Response":[{"RequestInquiry":{"id":21,"status":"PENDING","amount_inquired":{"currency":"EUR","value":"4.00"}}}]}"#,
        )
        .expect("request inquiry");
        assert_eq!(inquiry.response[0].id, 21);
        assert_eq!(inquiry.response[0].status.as_deref(), Some("PENDING"));
        let batch = decode_response::<PaymentBatch>(
            br#"{"Response":[{"PaymentBatch":{"id":22,"status":"PENDING"}}]}"#,
        )
        .expect("payment batch");
        assert_eq!(batch.response[0].id, 22);
    }
}
