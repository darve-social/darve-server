use std::collections::HashMap;

use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize)]
pub struct AccountLink {
    pub account: String,
    pub url: String,
}

#[derive(Debug, Deserialize)]
pub struct Account {
    pub id: String,
    pub contact_email: String,
}

#[derive(Debug, Deserialize)]
pub struct Event {
    pub id: String,
    pub object: String,
    pub context: Option<serde_json::Value>,
    pub created: String,
    pub data: std::collections::HashMap<String, serde_json::Value>,
    pub livemode: bool,
    pub reason: Option<serde_json::Value>,
    #[serde(rename = "type")]
    pub event_type: EventType,
}

#[derive(Debug, Deserialize, Serialize, Eq, PartialEq, Hash)]
pub enum EventType {
    #[serde(rename = "v2.core.account_link.completed")]
    AccountLinkCompleted,
    #[serde(rename = "v2.money_management.outbound_payment.canceled")]
    OutboundPaymentCanceled,
    #[serde(rename = "v2.money_management.outbound_payment.failed")]
    OutboundPaymentFailed,
    #[serde(rename = "v2.money_management.outbound_payment.returned")]
    OutboundPaymentReturned,
    #[serde(rename = "v2.money_management.outbound_payment.posted")]
    OutboundPaymentPosted,
}

#[derive(Debug, Deserialize)]
pub struct FinanceAccount {
    pub id: String,
    pub status: FinanceAccountStatus,
    pub balance: Balance,
}

#[derive(Debug, Deserialize)]
pub struct FinanceCurrencyValue {
    pub value: u64,
    pub currency: String,
}

#[derive(Debug, Deserialize)]
pub struct Balance {
    pub available: HashMap<String, FinanceCurrencyValue>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FinanceAccountStatus {
    Open,
    Close,
}

#[derive(Debug, Deserialize)]
pub struct OutboundPaymentResponse {
    pub id: String,
}
