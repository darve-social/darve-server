use serde::{Deserialize, Serialize};
use totp_rs::{Algorithm, Secret, TOTP};

#[derive(Debug, Serialize, Deserialize)]
pub struct TotpResponse {
    pub token: String,
    pub url: String,
    pub secret: String,
}

pub struct Totp {
    client: TOTP,
}

impl Totp {
    pub fn new(issuer: &str, secret: Option<String>) -> Self {
        let secret = match secret {
            Some(v) => Secret::Encoded(v),
            None => Secret::generate_secret(),
        };
        let client = TOTP::new(
            Algorithm::SHA1,
            6,
            1,
            30,
            secret.to_bytes().unwrap(),
            Some("Darve".to_string()),
            issuer.to_string(),
        )
        .unwrap();

        Self { client }
    }

    pub fn generate(&self) -> TotpResponse {
        let token = self.client.generate_current().unwrap();
        TotpResponse {
            token,
            url: self.client.get_url(),
            secret: self.client.get_secret_base32(),
        }
    }

    pub fn is_valid(&self, token: &str) -> bool {
        self.client.check_current(token).unwrap_or(false)
    }
}
