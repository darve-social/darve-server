use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
};
use middleware::error::CtxError;

use crate::middleware;

#[derive(Debug)]
pub enum WebauthnError {
    Unknown,
    CorruptSession,
    UserNotFound,
    UserExists,
    UserHasNoCredentials,
    WebauthnApiError(String),
    WebauthnNotImplemented,
}
impl IntoResponse for WebauthnError {
    fn into_response(self) -> Response {
        let body = match self {
            WebauthnError::CorruptSession => "Corrupt Session",
            WebauthnError::UserNotFound => "User Not Found",
            WebauthnError::Unknown => "Unknown Error",
            WebauthnError::UserHasNoCredentials => "User Has No Credentials",
            WebauthnError::UserExists => "User already exists",
            WebauthnError::WebauthnApiError(s) => {
                println!("WA ERR={}", s);
                "WebauthnApi error"
            }
            WebauthnError::WebauthnNotImplemented => {
                "Webauthn not implemented - probably loggedin/existing user"
            }
        };

        // its often easiest to implement `IntoResponse` by calling other implementations
        (StatusCode::INTERNAL_SERVER_ERROR, body).into_response()
    }
}

impl From<CtxError> for WebauthnError {
    fn from(value: CtxError) -> Self {
        WebauthnError::WebauthnApiError(value.error.to_string())
    }
}

impl From<tower_sessions::session::Error> for WebauthnError {
    fn from(_value: tower_sessions::session::Error) -> Self {
        WebauthnError::CorruptSession
    }
}
