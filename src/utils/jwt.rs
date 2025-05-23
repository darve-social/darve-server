use chrono::{TimeDelta, Utc};
use jsonwebtoken::{decode, encode, Algorithm, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};

pub struct JWTConfig {
    pub secret: String,
    pub duration: TimeDelta,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Claims {
    pub sub: String,
    pub auth: String,
    pub exp: usize,
    pub iat: usize,
}

pub struct JWT {
    key_enc: EncodingKey,
    key_dec: DecodingKey,
    duration: TimeDelta,
}

impl JWT {
    pub fn new(secret: String, duration: TimeDelta) -> Self {
        Self {
            duration,
            key_enc: EncodingKey::from_secret(secret.as_ref()),
            key_dec: DecodingKey::from_secret(secret.as_ref()),
        }
    }

    pub fn encode(&self, user_id: &String) -> Result<String, String> {
        let claims = Claims {
            sub: user_id.clone(),
            auth: user_id.clone(),
            exp: (Utc::now() + self.duration).timestamp() as usize,
            iat: Utc::now().timestamp() as usize,
        };

        let token_res = encode(&Header::default(), &claims, &self.key_enc);

        match token_res {
            Ok(token) => Ok(token),
            Err(err) => Err(err.to_string()),
        }
    }

    pub fn decode(&self, token: &str) -> Result<Claims, String> {
        let token_message =
            decode::<Claims>(&token, &self.key_dec, &Validation::new(Algorithm::HS256));

        match token_message {
            Ok(data) => Ok(data.claims),
            Err(err) => return Err(err.to_string()),
        }
    }
}
