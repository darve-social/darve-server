use reqwest::Client;
use serde::Deserialize;
use serde_json::json;

use crate::utils::stripe::models::OutboundPaymentResponse;

#[derive(Debug, Deserialize)]
pub struct FromAccount<'a> {
    pub finance_account: &'a str,
    pub currency: &'a str,
}

#[derive(Debug, Deserialize)]
pub struct ToAccount<'a> {
    pub account: &'a str,
    pub currency: &'a str,
}

#[derive(Debug, Deserialize)]
pub struct Amount<'a> {
    pub amount: f64,
    pub currency: &'a str,
}

pub struct OutboundPayment<'a> {
    version: &'a str,
    secret_key: &'a str,
}

impl<'a> OutboundPayment<'a> {
    pub fn new(version: &'a str, secret_key: &'a str) -> Self {
        Self {
            version,
            secret_key,
        }
    }

    pub async fn create(
        &self,
        from: FromAccount<'a>,
        to: ToAccount<'a>,
        amount: Amount<'a>,
        description: Option<&str>,
    ) -> Result<OutboundPaymentResponse, String> {
        let data = json!({
            "from":  {
                "financial_account": from.finance_account,
                "currency": from.currency
            },
            "to": {
                "recipient": to.account,
                "currency": to.currency
            },
            "amount": {
                "value": amount.amount,
                "currency": amount.currency
            },
            "description": description
        });

        let client = Client::new();
        let res = client
            .post("https://api.stripe.com/v2/money_management/outbound_payments")
            .header("Stripe-Version", self.version)
            .bearer_auth(&self.secret_key)
            .json(&data)
            .send()
            .await
            .map_err(|e| e.to_string())?;

        if !res.status().is_success() {
            return Err(res.text().await.map_err(|e| e.to_string())?);
        };

        let data = res
            .json::<OutboundPaymentResponse>()
            .await
            .map_err(|e| e.to_string())?;

        Ok(data)
    }
}
