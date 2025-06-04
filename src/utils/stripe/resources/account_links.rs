use reqwest::Client;
use serde_json::json;

use crate::utils::stripe::models::AccountLink;

pub struct AccountLinks<'a> {
    version: &'a str,
    secret_key: &'a str,
}

impl<'a> AccountLinks<'a> {
    pub fn new(version: &'a str, secret_key: &'a str) -> Self {
        Self {
            version,
            secret_key,
        }
    }

    pub async fn create_onboarding(
        &self,
        account: &str,
        refresh_url: &str,
        return_url: &str,
    ) -> Result<AccountLink, String> {
        let link_data = json!({
            "account": account,
            "use_case": {
                "type": "account_onboarding",
                "account_onboarding": {
                    "configurations": [
                        "recipient"
                    ],
                    "refresh_url": refresh_url,
                    "return_url": return_url
                }
            }
        });

        let res = Client::new()
            .post("https://api.stripe.com/v2/core/account_links")
            .header("Stripe-Version", self.version)
            .bearer_auth(&self.secret_key)
            .json(&link_data)
            .send()
            .await
            .map_err(|e| e.to_string())?;

        if !res.status().is_success() {
            return Err(res.text().await.map_err(|e| e.to_string())?);
        };

        let data = res.json::<AccountLink>().await.map_err(|e| e.to_string())?;
        Ok(data)
    }
}
