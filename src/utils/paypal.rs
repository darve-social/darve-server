use axum::{body::Bytes, http::HeaderMap};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::{from_slice, json};
use uuid::Uuid;

#[derive(Debug, Deserialize)]
struct VerifySignatureResponse {
    verification_status: String,
}

#[derive(Debug, Deserialize)]
struct AccessTokenResponse {
    access_token: String,
}

#[derive(Serialize)]
struct VerifySignatureRequest<'a> {
    auth_algo: &'a str,
    cert_url: &'a str,
    transmission_id: &'a str,
    transmission_sig: &'a str,
    transmission_time: &'a str,
    webhook_id: &'a str,
    webhook_event: &'a WebhookEvent,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct WebhookEvent {
    pub event_type: String,
    pub resource: serde_json::Value,
}

pub struct Paypal<'a> {
    client_key: &'a str,
    client_id: &'a str,
    webhook_id: &'a str,
}

impl<'a> Paypal<'a> {
    pub fn new(client_id: &'a str, client_key: &'a str, webhook_id: &'a str) -> Self {
        Self {
            client_id,
            client_key,
            webhook_id,
        }
    }
    pub async fn get_event_from_request(
        &self,
        headers: HeaderMap,
        body: Bytes,
    ) -> Result<WebhookEvent, &'static str> {
        let event: WebhookEvent = from_slice(&body).expect("Paypal webhook event from slice error");

        let sig = headers
            .get("paypal-transmission-sig")
            .unwrap()
            .to_str()
            .unwrap();
        let time = headers
            .get("paypal-transmission-time")
            .unwrap()
            .to_str()
            .unwrap();
        let id = headers
            .get("paypal-transmission-id")
            .unwrap()
            .to_str()
            .unwrap();

        let cert_url = headers.get("paypal-cert-url").unwrap().to_str().unwrap();
        let algo = headers.get("paypal-auth-algo").unwrap().to_str().unwrap();
        let payload = VerifySignatureRequest {
            auth_algo: algo,
            cert_url,
            transmission_id: id,
            transmission_sig: sig,
            transmission_time: time,
            webhook_id: &self.webhook_id,
            webhook_event: &event,
        };
        let res = Client::new()
            .post("https://api-m.sandbox.paypal.com/v1/notifications/verify-webhook-signature")
            .bearer_auth(&self.client_key)
            .json(&payload)
            .send()
            .await
            .unwrap();

        let verify_res: VerifySignatureResponse = res.json().await.unwrap();
        println!("Verification status: {}", verify_res.verification_status);

        if verify_res.verification_status == "SUCCESS" {
            Ok(event)
        } else {
            Err("Paypal verification failed")
        }
    }

    pub async fn send_money(&self, email: &str, amount: f64, currency: &str) -> Result<(), String> {
        if amount.le(&1.0) {
            return Err("Amount must not be less than 1".to_string());
        };

        let payout = json!({
            "sender_batch_header":{
                "sender_batch_id": Uuid::new_v4().to_string(),
                "email_subject": "You have a payout!".to_string(),
                "email_message": "Thanks for your work!".to_string(),
            },
            "items": [{
                "recipient_type": "EMAIL".to_string(),
                "amount":{
                    "value": amount.to_string(),
                    "currency": currency.to_string(),
                },
                "receiver": email,
            }],
        });

        let access_token = self.get_access_token().await?;

        let res = Client::new()
            .post("https://api-m.sandbox.paypal.com/v1/payments/payouts")
            .bearer_auth(&access_token)
            .json(&payout)
            .send()
            .await
            .map_err(|e| e.to_string())?;

        Ok(())
    }

    async fn get_access_token(&self) -> Result<String, String> {
        let res = Client::new()
            .post("https://api-m.sandbox.paypal.com/v1/oauth2/token")
            .basic_auth(self.client_id, Some(self.client_key))
            .form(&[("grant_type", "client_credentials")])
            .send()
            .await
            .map_err(|e| e.to_string())?;

        let token_response = res
            .json::<AccessTokenResponse>()
            .await
            .map_err(|e| e.to_string())?;
        Ok(token_response.access_token)
    }
}
