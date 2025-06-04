use reqwest::Client;
use serde::Deserialize;

use crate::utils::stripe::models::FinanceAccount;

#[derive(Debug, Deserialize)]
struct GetResponse {
    data: Vec<FinanceAccount>,
}

pub struct FinanceAccounts<'a> {
    version: &'a str,
    secret_key: &'a str,
}

impl<'a> FinanceAccounts<'a> {
    pub fn new(version: &'a str, secret_key: &'a str) -> Self {
        Self {
            version,
            secret_key,
        }
    }

    pub async fn get(&self) -> Result<Vec<FinanceAccount>, String> {
        let result = Client::new()
            .get("https://api.stripe.com/v2/money_management/financial_accounts")
            .header("Stripe-Version", self.version)
            .bearer_auth(&self.secret_key)
            .send()
            .await
            .map_err(|e| e.to_string())?;

        if !result.status().is_success() {
            return Err(result.text().await.map_err(|e| e.to_string())?);
        };

        let accounts = result
            .json::<GetResponse>()
            .await
            .map_err(|e| e.to_string())?;

        Ok(accounts.data)
    }
}
