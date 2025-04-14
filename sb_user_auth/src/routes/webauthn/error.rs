use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
};
use sb_middleware::error::CtxError;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum WebauthnError {
    #[error("unknown webauthn error")]
    Unknown,
    #[error("Corrupt Session")]
    CorruptSession,
    #[error("User Not Found")]
    UserNotFound,
    #[error("User Already Exists")]
    UserExists,
    #[error("User Has No Credentials")]
    UserHasNoCredentials,
    #[error("Api error in webauthn ={0}")]
    WebauthnApiError(String),
    #[error("Api not implemented")]
    WebauthnNotImplemented,
    #[error("Deserialising Session failed: {0}")]
    InvalidSessionState(#[from] tower_sessions::session::Error),
}
impl IntoResponse for WebauthnError {
    fn into_response(self) -> Response {
        let body = match self {
            WebauthnError::CorruptSession => "Corrupt Session",
            WebauthnError::UserNotFound => "User Not Found",
            WebauthnError::Unknown => "Unknown Error",
            WebauthnError::UserHasNoCredentials => "User Has No Credentials",
            WebauthnError::InvalidSessionState(_) => "Deserialising Session failed",
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
