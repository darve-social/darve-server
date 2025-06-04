use reqwest::Client;

use crate::utils::stripe::models::Event;

pub struct Events<'a> {
    version: &'a str,
    secret_key: &'a str,
}

impl<'a> Events<'a> {
    pub fn new(version: &'a str, secret_key: &'a str) -> Self {
        Self {
            version,
            secret_key,
        }
    }

    pub async fn get(&self, event_id: &str) -> Result<Event, String> {
        let res = Client::new()
            .get(&format!("https://api.stripe.com/v2/core/events/{event_id}"))
            .header("Stripe-Version", self.version)
            .bearer_auth(&self.secret_key)
            .send()
            .await
            .map_err(|e| e.to_string())?;

        if !res.status().is_success() {
            return Err(res.text().await.map_err(|e| e.to_string())?);
        };

        let data = res.json::<Event>().await.map_err(|e| e.to_string())?;
        Ok(data)
    }
}
