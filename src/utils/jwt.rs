use chrono::{Duration, TimeDelta, Utc};
use jsonwebtoken::{decode, encode, Algorithm, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};

pub struct JWTConfig {
    pub secret: String,
    pub duration: TimeDelta,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum TokenType {
    Login,
    Otp,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Claims {
    pub sub: String,
    pub auth: String,
    pub exp: usize,
    pub iat: usize,
    pub r#type: TokenType,
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

    pub fn create_by_login(&self, user_id: &str) -> Result<String, String> {
        let claims = Claims {
            sub: user_id.to_string(),
            auth: user_id.to_string(),
            exp: (Utc::now() + self.duration).timestamp() as usize,
            iat: Utc::now().timestamp() as usize,
            r#type: TokenType::Login,
        };

        let token_res = encode(&Header::default(), &claims, &self.key_enc);

        match token_res {
            Ok(token) => Ok(token),
            Err(err) => Err(err.to_string()),
        }
    }

    pub fn create_by_otp(&self, user_id: &str) -> Result<String, String> {
        let claims = Claims {
            sub: user_id.to_string(),
            auth: user_id.to_string(),
            exp: (Utc::now() + Duration::minutes(1)).timestamp() as usize,
            iat: Utc::now().timestamp() as usize,
            r#type: TokenType::Login,
        };

        let token_res = encode(&Header::default(), &claims, &self.key_enc);

        match token_res {
            Ok(token) => Ok(token),
            Err(err) => Err(err.to_string()),
        }
    }

    pub fn decode_by_type(&self, token: &str, r#type: TokenType) -> Result<Claims, String> {
        let token_message =
            decode::<Claims>(&token, &self.key_dec, &Validation::new(Algorithm::HS256));

        let data = match token_message {
            Ok(data) => data.claims,
            Err(err) => return Err(err.to_string()),
        };

        if data.r#type == r#type {
            Ok(data)
        } else {
            Err("Token type is not equal".to_string())
        }
    }
}
