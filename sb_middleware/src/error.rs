use std::fmt;

use axum::{http::StatusCode, response::IntoResponse};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::ctx::Ctx;

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct CtxError {
    pub error: AppError,
    pub req_id: Uuid,
    pub is_htmx: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum AppError {
    Generic { description: String },
    AuthenticationFail,
    RegisterFail,
    AuthorizationFail{required: String},
    EntityFailIdNotFound { ident: String },
    AuthFailNoJwtCookie,
    AuthFailJwtInvalid { source: String },
    AuthFailCtxNotInRequestExt,
    Serde { source: String },
    Stripe { source: String },
    SurrealDb { source: String },
    SurrealDbNoResult { source: String, id: String },
    SurrealDbParse { source: String, id: String },
}

/// ApiError has to have the req_id to report to the client and implements IntoResponse.
pub type CtxResult<T> = core::result::Result<T, CtxError>;
/// Any error for storing before composing a response.
/// For errors that either don't affect the response, or are build before attaching the req_id.
pub type AppResult<T> = core::result::Result<T, AppError>;

impl std::error::Error for AppError {}
// We don't implement Error for ApiError, because it doesn't implement Display.
// Implementing Display for it triggers a generic impl From ApiError for gql-Error on async-graphql - and we want to implement it ourselves, to always include extensions on Errors. It would create conflicting implementations.

// for slightly less verbose error mappings
impl CtxError {
    pub fn from<T: Into<AppError>>(ctx: &Ctx) -> impl FnOnce(T) -> CtxError + '_ {
        |err| {
            let error = err.into();
            CtxError {
                req_id: ctx.req_id(),
                error: error,
                is_htmx: ctx.is_htmx,
            }
        }
    }
}

impl From<surrealdb::Error> for CtxError {
    fn from(value: surrealdb::Error) -> Self {
        dbg!(&value);
        CtxError {
            req_id: Uuid::new_v4(),
            error: value.into(),
            is_htmx: false,
        }
    }
}

// TODO remove this and map_error in entity for db_utils errors
impl From<AppError> for CtxError {
    fn from(value: AppError) -> Self {
        CtxError {
            req_id: Uuid::new_v4(),
            error: value,
            is_htmx: false,
        }
    }
}

const INTERNAL: &str = "Internal error";

impl fmt::Display for AppError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Generic { description } => write!(f, "{description}"),
            Self::AuthenticationFail => write!(f, "Authentication failed"),
            Self::RegisterFail => write!(f, "Register fail"),
            Self::EntityFailIdNotFound { ident: id } => write!(f, "Record id= {id} not found"),
            Self::AuthFailNoJwtCookie => write!(f, "You are not logged in"),
            Self::AuthFailJwtInvalid { .. } => {
                write!(f, "The provided JWT token is not valid")
            }
            Self::Serde { source } => write!(f, "Serde error - {source}"),
            Self::AuthFailCtxNotInRequestExt => write!(f, "{INTERNAL}"),
            Self::SurrealDb { .. } => write!(f, "{INTERNAL}"),
            Self::SurrealDbNoResult { id, .. } => write!(f, "No result for id {id}"),
            Self::SurrealDbParse { id, .. } => write!(f, "Couldn't parse id {id}"),
            AppError::AuthorizationFail { .. } => write!(f, "not authorized"),
            AppError::Stripe { .. } => write!(f, "Stripe error")
        }
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct ErrorResponseBody {
    error: String,
    req_id: String,
}

impl ErrorResponseBody {
    pub fn new(error: String, req_id: Option<String>) -> Self {
        ErrorResponseBody {
            error,
            req_id: req_id.unwrap_or_else(|| Uuid::new_v4().to_string()),
        }
    }

    pub fn get_err(&self) -> String {
        self.error.clone()
    }
}

impl From<ErrorResponseBody> for String {
    fn from(value: ErrorResponseBody) -> Self {
        serde_json::to_string(&value).unwrap()
    }
}

// REST error response
impl IntoResponse for CtxError {
    fn into_response(self) -> axum::response::Response {
        println!("->> {:<12} - into_response - {self:?}", "ERROR");
        let status_code = match self.error {
            AppError::EntityFailIdNotFound { .. } => StatusCode::NOT_FOUND,
            AppError::Serde { .. }
            | AppError::SurrealDbNoResult { .. }
            | AppError::SurrealDbParse { .. }
            | AppError::Generic { .. }
            | AppError::Stripe { .. }
            | AppError::SurrealDb { .. } => StatusCode::BAD_REQUEST,
            AppError::AuthenticationFail
            | AppError::RegisterFail
            | AppError::AuthFailNoJwtCookie
            | AppError::AuthFailJwtInvalid { .. }
            | AppError::AuthorizationFail { .. }
            | AppError::AuthFailCtxNotInRequestExt => StatusCode::FORBIDDEN,
        };
        let err = self.error.clone();
        let bodyStr = get_error_body(&self, self.is_htmx);
        let mut response = (status_code, bodyStr.to_string()).into_response();
        // Insert the real Error into the response - for the logger
        response.extensions_mut().insert(err);
        response
    }
}


fn get_error_body(err: &CtxError, is_htmx: bool) -> String {
    match is_htmx {
        true => to_err_html(err.error.to_string()),
        false => ErrorResponseBody::new(err.error.to_string(), Some(err.req_id.to_string())).into()
    }
}

pub fn to_err_html(err_str: String) -> String {
    let mut ret_html = "<div>".to_string();
    let found = err_str.find("\n");
    let lines = err_str.split("\n");
    if found.is_some() {
        ret_html += "<ul>";
        for line in lines.into_iter() {
            ret_html += format!("<li>{line}</li>").as_str();
        }
        ret_html += "</ul>"
    } else {
        ret_html += err_str.as_str();
    }
    ret_html += "</div>";
    ret_html
}

// for sending serialized keys through gql extensions
pub const ERROR_SER_KEY: &str = "error_ser";

// External Errors
impl From<serde_json::Error> for AppError {
    fn from(value: serde_json::Error) -> Self {
        Self::Serde {
            source: value.to_string(),
        }
    }
}

impl From<surrealdb::Error> for AppError {
    fn from(value: surrealdb::Error) -> Self {
        Self::SurrealDb {
            source: value.to_string(),
        }
    }
}

impl From<stripe::StripeError> for AppError {
    fn from(value: stripe::StripeError) -> Self {
        Self::Stripe {
            source: value.to_string(),
        }
    }
}

impl From<CtxError> for AppError {
    fn from(value: CtxError) -> Self {
        value.error
    }
}

/*impl From<ApiError> for Response<Body> {
    fn from(value: ApiError) -> Self {
        match value.error {
            Error::LoginFail => (StatusCode::UNAUTHORIZED, "login failed").into_response(),
            Error::RegisterFail =>(StatusCode::CONFLICT, "register failed").into_response(),
            Error::EntityFailIdNotFound { .. } => (StatusCode::NOT_FOUND, "entity not found").into_response(),
            _ => (StatusCode::INTERNAL_SERVER_ERROR, "generic error").into_response(),
            _ => value.error.into_response(),
        }
    }
}
*/
impl From<jsonwebtoken::errors::Error> for AppError {
    fn from(value: jsonwebtoken::errors::Error) -> Self {
        Self::AuthFailJwtInvalid {
            source: value.to_string(),
        }
    }
}
