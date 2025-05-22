use reqwest::Client;
use serde::Deserialize;

#[derive(Deserialize, Debug)]
pub struct FacebookUser {
    pub id: String,
    pub name: String,
    pub email: Option<String>,
}

pub async fn verify_token(access_token: &str) -> Option<FacebookUser> {
    let client = Client::new();
    let response = client
        .get("https://graph.facebook.com/me")
        .query(&[("fields", "id,name,email"), ("access_token", access_token)])
        .send()
        .await
        .ok();

    match response {
        Some(re) => re.json::<FacebookUser>().await.ok(),
        None => None,
    }
}
