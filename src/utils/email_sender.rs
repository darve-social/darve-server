use crate::interfaces::send_email::SendEmailInterface;
use async_trait::async_trait;

pub struct EmailSender {
    api_key: String,
    api_url: String,
    no_replay: String,
    client: reqwest::Client,
}

impl EmailSender {
    pub fn from_env() -> Self {
        let api_key = std::env::var("SENDGRID_API_KEY").expect("SENDGRID_API_KEY must be set");
        let no_replay = std::env::var("NO_REPLY_EMAIL").expect("NO_REPLY_EMAIL must be set");
        let api_url = std::env::var("SENDGRID_API_URL")
            .unwrap_or("https://api.sendgrid.com/v3/mail/send".to_string());
        Self {
            api_key,
            api_url,
            no_replay,
            client: reqwest::Client::new(),
        }
    }
}

#[async_trait]
impl SendEmailInterface for EmailSender {
    async fn send(&self, emails: Vec<String>, body: &str, subject: &str) -> Result<(), String> {
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

        let response = self
            .client
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
