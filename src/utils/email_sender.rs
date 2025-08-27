use crate::interfaces::send_email::SendEmailInterface;
use async_trait::async_trait;
use reqwest::Client;

pub struct EmailSender {
    api_key: String,
    api_url: String,
    no_replay: String,
}

impl EmailSender {
    pub fn new(api_key: &str, api_url: &str, no_replay: &str) -> Self {
        Self {
            api_key: api_key.to_string(),
            api_url: api_url.to_string(),
            no_replay: no_replay.to_string(),
        }
    }
}

#[async_trait]
impl SendEmailInterface for EmailSender {
    async fn send(&self, emails: Vec<String>, body: &str, subject: &str) -> Result<(), String> {
        // !!!TODO mock this for test
        if cfg!(test) {
            return Ok(());
        }

        let personalizations = vec![serde_json::json!({
            "to": emails.iter().map(|email| serde_json::json!({ "email": email })).collect::<Vec<_>>(),
        })];

        let payload = serde_json::json!({
            "personalizations": personalizations,
            "from": { "email": self.no_replay },
            "content": [{
                "type": "text/html",
                "value": body,
            }],
            "subject": subject,
        });

        let response = Client::new()
            .post(&self.api_url)
            .bearer_auth(&self.api_key)
            .header("Content-Type", "application/json")
            .json(&payload)
            .send()
            .await
            .map_err(|e| e.to_string())?;

        if response.status().is_success() {
            Ok(())
        } else {
            Err(format!(
                "Failed to send email: {}",
                response.text().await.unwrap_or_default()
            ))
        }
    }
}
