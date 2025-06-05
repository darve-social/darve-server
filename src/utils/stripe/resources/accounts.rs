use reqwest::Client;
use serde::Serialize;
use serde_json::json;

use crate::utils::stripe::models::Account;

#[derive(Debug, Serialize)]
pub struct Identity<'a> {
    pub country: &'a str,
    pub entity_type: Option<EntityType>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum EntityType {
    Individual,
}

pub struct Accounts<'a> {
    version: &'a str,
    secret_key: &'a str,
}

impl<'a> Accounts<'a> {
    pub fn new(version: &'a str, secret_key: &'a str) -> Self {
        Self {
            version,
            secret_key,
        }
    }

    pub async fn create(&self, email: &str, identity: Identity<'a>) -> Result<Account, String> {
        let data = json!({
            "contact_email": email,
            "configuration": {
                "recipient": {
                    "capabilities": {
                        "cards": {
                            "requested": true
                        }
                    }
                },
            },
            "identity": {
                "country": identity.country,
                "entity_type": identity.entity_type.unwrap_or(EntityType::Individual)
            },
            "dashboard": "full",
            "include": [
                "configuration.recipient",
                "identity",
                "defaults"
            ]
        });
        let res = Client::new()
            .post("https://api.stripe.com/v2/core/accounts")
            .header("Stripe-Version", self.version)
            .bearer_auth(&self.secret_key)
            .json(&data)
            .send()
            .await
            .map_err(|e| e.to_string())?;

        if !res.status().is_success() {
            return Err(res.text().await.map_err(|e| e.to_string())?);
        };

        let data = res.json::<Account>().await.map_err(|e| e.to_string())?;
        Ok(data)
    }

    pub async fn get(&self, id: &str) -> Result<Account, String> {
        let res = Client::new()
            .get(format!("https://api.stripe.com/v2/core/accounts/{id}"))
            .header("Stripe-Version", self.version)
            .bearer_auth(&self.secret_key)
            .send()
            .await
            .map_err(|e| e.to_string())?;

        println!(">>>>>>{:?}", res);
        if !res.status().is_success() {
            return Err(res.text().await.map_err(|e| e.to_string())?);
        };

        let data = res.json::<Account>().await.map_err(|e| e.to_string())?;
        Ok(data)
    }
}
