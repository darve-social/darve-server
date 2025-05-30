use reqwest::Client;
use serde::Deserialize;

#[derive(Deserialize, Debug)]
pub struct FacebookUser {
    pub id: String,
    pub name: String,
    pub email: Option<String>, // facebook return only verified email
}

pub async fn verify_token(access_token: &str) -> Result<FacebookUser, String> {
    let client = Client::new();
    let response = client
        .get("https://graph.facebook.com/me")
        .query(&[
            ("fields", "id, name, email"),
            ("access_token", access_token),
        ])
        .send()
        .await
        .map_err(|e| e.to_string())?;

    let user = response
        .json::<FacebookUser>()
        .await
        .map_err(|e| e.to_string())?;

    Ok(user)
}
