use chrono::{Duration, Utc};
use jsonwebtoken::{encode, EncodingKey, Header};
use tower_cookies::{Cookie, Cookies};

use crate::mw_ctx::{Claims, JWT_KEY};

pub fn issue_login_jwt(
    key_enc: &EncodingKey,
    cookies: Cookies,
    local_user_id: Option<String>,
    jwt_duration: Duration,
) {
    // NOTE: set to a reasonable number after testing
    // NOTE when testing: the default validation.leeway is 5min
    let exp = Utc::now() + jwt_duration;
    let claims = Claims {
        exp: exp.timestamp() as usize,
        auth: local_user_id.unwrap(),
    };
    let token_str = encode(&Header::default(), &claims, &key_enc).expect("JWT encode should work");

    cookies.add(
        Cookie::build((JWT_KEY, token_str))
            // if not set, the path defaults to the path from which it was called - prohibiting gql on root if login is on /api
            .path("/")
            .http_only(true)
            .into(), //.finish(),
    );
}
