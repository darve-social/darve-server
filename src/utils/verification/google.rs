use reqwest::Client;
use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct GoogleUser {
    pub sub: String,
    pub email: String,
    pub aud: String,
    pub name: Option<String>,
    pub picture: Option<String>,
}

pub async fn verify_token(token: &str, client_id: &str) -> Result<GoogleUser, String> {
    let url = format!("https://oauth2.googleapis.com/tokeninfo?id_token={}", token);
    let client = Client::new();
    let response = client
        .get(&url)
        .send()
        .await
        .map_err(|err| err.to_string())?
        .json::<GoogleUser>()
        .await
        .map_err(|err| err.to_string())?;

    if response.aud != client_id {
        return Err("Invalid token for this client ID".to_string());
    }

    Ok(response)
}
