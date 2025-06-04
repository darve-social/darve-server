use async_trait::async_trait;

use crate::utils::stripe::models::{Account, AccountLink, Event};

#[async_trait]
pub trait PaymentInterface {
    async fn recipient_link(
        &self,
        account: &str,
        refresh_url: &str,
        return_url: &str,
    ) -> Result<AccountLink, String>;
    async fn outbound_payments(
        &self,
        recipient_account: &str,
        amount_currency: &str,
        amount: f64,
        description: Option<&str>,
    ) -> Result<String, String>;
    async fn create_recipient_account(&self, email: &str, country: &str)
        -> Result<Account, String>;
    async fn get_event(&self, event_id: &str) -> Result<Event, String>;
    async fn get_account(&self, id: &str) -> Result<Account, String>;
}
