use reqwest::Client;
use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct GoogleUser {
    pub sub: String,
    pub email: Option<String>,
    aud: String,
    pub name: Option<String>,
    pub picture: Option<String>,
    email_verified: Option<String>,
}

pub async fn verify_token(token: &str, client_ids: &Vec<&str>) -> Result<GoogleUser, String> {
    let url = format!("https://oauth2.googleapis.com/tokeninfo?id_token={}", token);
    let client = Client::new();
    let mut user = client
        .get(&url)
        .send()
        .await
        .map_err(|err| err.to_string())?
        .json::<GoogleUser>()
        .await
        .map_err(|err| err.to_string())?;

    if !client_ids.contains(&user.aud.as_str()) {
        return Err("Invalid token for this client ID".to_string());
    }

    if user.email_verified.as_deref() != Some("true") {
        user.email = None
    }

    Ok(user)
}
