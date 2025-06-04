use crate::{
    interfaces::payment::PaymentInterface,
    utils::stripe::{
        models::{Account, AccountLink, Event},
        resources::{
            account_links::AccountLinks,
            accounts::{Accounts, Identity},
            events::Events,
            finance_accounts::FinanceAccounts,
            outbound_payments::{Amount, FromAccount, OutboundPayment, ToAccount},
        },
    },
};
use async_trait::async_trait;

pub mod models;
mod resources;
pub mod webhook;

pub struct StripePayment {
    secret_key: String,
    version: &'static str,
}

impl StripePayment {
    pub fn new(secret_key: String) -> Self {
        Self {
            secret_key,
            version: "2025-05-28.preview",
        }
    }
}

#[async_trait]
impl PaymentInterface for StripePayment {
    async fn recipient_link(
        &self,
        account: &str,
        refresh_url: &str,
        return_url: &str,
    ) -> Result<AccountLink, String> {
        let account_links = AccountLinks::new(self.version, &self.secret_key);
        account_links
            .create_onboarding(account, refresh_url, return_url)
            .await
    }

    async fn outbound_payments(
        &self,
        recipient_account: &str,
        amount_currency: &str,
        amount: f64,
        description: Option<&str>,
    ) -> Result<String, String> {
        let finance_accounts = FinanceAccounts::new(self.version, &self.secret_key);
        let accounts = finance_accounts.get().await?;

        if accounts.is_empty() {
            return Err("Finance Accounts has't been found".to_string());
        }

        let finance_account = &accounts.first().unwrap().id;
        let outbound_payments = OutboundPayment::new(self.version, &self.secret_key);
        let result = outbound_payments
            .create(
                FromAccount {
                    finance_account,
                    currency: "usd",
                },
                ToAccount {
                    account: recipient_account,
                    currency: "usd",
                },
                Amount {
                    currency: amount_currency,
                    amount,
                },
                description,
            )
            .await?;
        Ok(result.id)
    }

    async fn create_recipient_account(
        &self,
        email: &str,
        country: &str,
    ) -> Result<Account, String> {
        let accounts = Accounts::new(self.version, &self.secret_key);
        accounts
            .create(
                email,
                Identity {
                    country,
                    entity_type: None,
                },
            )
            .await
    }

    async fn get_event(&self, event_id: &str) -> Result<Event, String> {
        let events = Events::new(self.version, &self.secret_key);
        events.get(event_id).await
    }
    async fn get_account(&self, id: &str) -> Result<Account, String> {
        let accounts = Accounts::new(self.version, &self.secret_key);
        accounts.get(id).await
    }
}
