use reqwest::Client;
use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize, Debug)]
pub struct TwitchUser {
    pub id: String,
    pub login: String,
    pub display_name: String,
    pub email: Option<String>,
    pub profile_image_url: Option<String>,
}

#[derive(Deserialize, Serialize, Debug)]
struct TwitchUsersResponse {
    data: Vec<TwitchUser>,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct TwitchTokenResponse {
    pub access_token: String,
    pub token_type: String,
    pub refresh_token: String,
    pub expires_in: u64,
}

pub struct TwitchService {
    client_id: String,
    client_secret: String,
    redirect_uri: String,
}

impl TwitchService {
    pub fn new(client_id: String, client_secret: String, redirect_uri: String) -> Self {
        Self {
            client_id,
            client_secret,
            redirect_uri,
        }
    }

    pub async fn exchange_code(&self, code: &str) -> Result<TwitchTokenResponse, String> {
        let client = Client::new();
        let response = client
            .post("https://id.twitch.tv/oauth2/token")
            .form(&[
                ("client_id", self.client_id.as_str()),
                ("client_secret", self.client_secret.as_str()),
                ("code", code),
                ("grant_type", "authorization_code"),
                ("redirect_uri", self.redirect_uri.as_str()),
            ])
            .send()
            .await
            .map_err(|e| e.to_string())?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(format!(
                "Twitch token exchange error: {} - {}",
                status, body
            ));
        }

        let token = response
            .json::<TwitchTokenResponse>()
            .await
            .map_err(|e| e.to_string())?;

        println!("Twitch token exchange success: {:?}", token);
        Ok(token)
    }

    pub async fn get_user(&self, token: &TwitchTokenResponse) -> Result<TwitchUser, String> {
        let client = Client::new();
        let response = client
            .get("https://api.twitch.tv/helix/users")
            .header("Authorization", format!("Bearer {}", token.access_token))
            .header("Client-Id", self.client_id.as_str())
            .send()
            .await
            .map_err(|e| e.to_string())?;

        if !response.status().is_success() {
            return Err(format!("Twitch API error: {}", response.status()));
        }

        let users_response = response
            .json::<TwitchUsersResponse>()
            .await
            .map_err(|e| e.to_string())?;

        users_response
            .data
            .into_iter()
            .next()
            .ok_or_else(|| "No user data returned from Twitch".to_string())
    }

    pub async fn refresh_token(
        &self,
        token: &TwitchTokenResponse,
    ) -> Result<TwitchTokenResponse, String> {
        let client = Client::new();

        let response = client
            .get("https://id.twitch.tv/oauth2/validate")
            .header("Authorization", format!("Bearer {}", token.access_token))
            .send()
            .await
            .map_err(|e| e.to_string())?;

        if response.status().is_success() {
            return Ok(token.clone());
        }

        let response = client
            .post("https://id.twitch.tv/oauth2/token")
            .form(&[
                ("client_id", self.client_id.as_str()),
                ("client_secret", self.client_secret.as_str()),
                ("grant_type", "refresh_token"),
                ("refresh_token", token.refresh_token.as_str()),
            ])
            .send()
            .await
            .map_err(|e| e.to_string())?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(format!("Twitch token refresh error: {} - {}", status, body));
        }

        response
            .json::<TwitchTokenResponse>()
            .await
            .map_err(|e| e.to_string())
    }
}
