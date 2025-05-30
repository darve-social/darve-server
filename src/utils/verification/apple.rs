use jsonwebtoken::{decode, decode_header, Algorithm, DecodingKey, Validation};
use reqwest;
use serde::Deserialize;
use std::collections::HashMap;

#[derive(Debug, Deserialize)]
struct AppleIdTokenClaims {
    iss: String, // Issuer - should be "https://appleid.apple.com"
    sub: String, // Subject - user's unique ID from Apple
    aud: String, // Audience - your client_id/app_id
    exp: usize,  // Expiration time
    #[serde(skip_serializing_if = "Option::is_none")]
    email: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    email_verified: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    is_private_email: Option<bool>,
}

// Apple's JWKS response structure
#[derive(Debug, Deserialize)]
struct JwksResponse {
    keys: Vec<JwkKey>,
}

#[derive(Debug, Deserialize)]
struct JwkKey {
    kid: String,
    n: String,
    e: String,
}

#[derive(Debug)]
pub struct AppleUserInfo {
    pub id: String,
    pub email: Option<String>,
    pub name: Option<String>,
}

async fn get_apple_jwks() -> Result<HashMap<String, DecodingKey>, String> {
    let jwks_url = "https://appleid.apple.com/auth/keys";
    let response = reqwest::get(jwks_url)
        .await
        .map_err(|e| e.to_string())?
        .json::<JwksResponse>()
        .await
        .map_err(|e| e.to_string())?;
    let mut keys = HashMap::new();
    for jwk in response.keys {
        // Create a decoding key from the JWK
        let key = DecodingKey::from_rsa_components(&jwk.n, &jwk.e).map_err(|e| e.to_string())?;
        keys.insert(jwk.kid, key);
    }

    Ok(keys)
}

pub async fn verify_token(token: &str, client_id: &str) -> Result<AppleUserInfo, String> {
    // 1. Decode the token header to get the kid (key ID)
    let header = decode_header(token).map_err(|e| e.to_string())?;
    let kid = header.kid.ok_or("No 'kid' found in token header")?;

    // 2. Fetch Apple's JWKS
    let keys = get_apple_jwks().await?;

    // 3. Get the correct key for this token
    let decoding_key = keys.get(&kid).ok_or("No matching key found for token")?;

    // 4. Set up validation criteria
    let mut validation = Validation::new(Algorithm::RS256);
    validation.set_audience(&[client_id]);
    validation.set_issuer(&["https://appleid.apple.com"]);

    // 5. Verify and decode the token
    let token_data = decode::<AppleIdTokenClaims>(token, &decoding_key, &validation)
        .map_err(|e| e.to_string())?;
    let claims = token_data.claims;

    // 6. Additional validation checks
    if claims.iss != "https://appleid.apple.com" {
        return Err("Invalid issuer".into());
    }

    if claims.aud != client_id {
        return Err("Invalid audience".into());
    }

    let current_time = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|e| e.to_string())?
        .as_secs() as usize;

    if claims.exp < current_time {
        return Err("Token has expired".into());
    }

    let is_verified = claims.is_private_email != Some(true) && claims.email_verified == Some(true);

    Ok(AppleUserInfo {
        id: claims.sub,
        email: if is_verified { claims.email } else { None },
        name: None,
    })
}
