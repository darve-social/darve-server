use totp_rs::{Algorithm, Secret, TOTP};

#[derive(Debug)]
pub struct TotpResposne {
    pub token: String,
    pub url: String,
    pub secret: String,
}

pub struct Totp {
    key: String,
    app_name: Option<String>,
}

impl Totp {
    pub fn new(key: String) -> Self {
        Self {
            key,
            app_name: Some("Darve".to_string()),
        }
    }

    pub fn generate(&self, issuer: &str) -> TotpResposne {
        let totp = self.build(issuer);
        let token = totp.generate_current().unwrap();
        let url = totp.get_url();
        TotpResposne {
            token,
            url,
            secret: totp.get_secret_base32(),
        }
    }

    pub fn is_valid(&self, issuer: &str, token: &str) -> bool {
        self.build(issuer).check_current(token).unwrap_or(false)
    }

    fn build(&self, issuer: &str) -> TOTP {
        let secret = Secret::Encoded(self.key.clone()).to_bytes().unwrap();
        TOTP::new(
            Algorithm::SHA1,
            6,
            1,
            30,
            secret,
            self.app_name.clone(),
            issuer.to_string(),
        )
        .unwrap()
    }
}
