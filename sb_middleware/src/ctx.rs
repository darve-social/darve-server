use crate::error::*;
use askama::Template;
use axum::extract::FromRequestParts;
use axum::response::Html;
use serde::Serialize;
use uuid::Uuid;

#[derive(Clone, Debug)]
pub struct Ctx {
    result_user_id: AppResult<String>,
    req_id: Uuid,
    pub is_htmx: bool,
}

impl Ctx {
    pub fn new(result_user_id: AppResult<String>, uuid: Uuid, is_htmx: bool) -> Self {
        Self {
            result_user_id,
            req_id: uuid,
            is_htmx,
        }
    }

    pub fn user_id(&self) -> CtxResult<String> {
        self.result_user_id.clone().map_err(|error| CtxError {
            error,
            req_id: self.req_id,
            is_htmx: self.is_htmx,
        })
    }

    pub fn req_id(&self) -> Uuid {
        self.req_id
    }

    pub fn to_htmx_or_json<T: Template + Serialize>(&self, object: T) -> CtxResult<Html<String>> {
        let rendered_string = match self.is_htmx {
            true => object.render().map_err(|e| {
                self.to_ctx_error(AppError::Generic {
                    description: "Render template error".to_string(),
                })
            })?,
            false => serde_json::to_string(&object).map_err(|e| {
                self.to_ctx_error(AppError::Generic {
                    description: "Render json error".to_string(),
                })
            })?,
        };
        Ok(Html(rendered_string))
    }

    pub fn to_ctx_error(&self, error: AppError) -> CtxError {
        CtxError {
            is_htmx: self.is_htmx,
            req_id: self.req_id,
            error,
        }
    }
}

// ugly but direct implementation from axum, until "async trait fn" are in stable rust, instead of importing some 3rd party macro
// Extractor - makes it possible to specify Ctx as a param - fetches the result from the header parts extension
impl<S: Send + Sync> FromRequestParts<S> for Ctx {
    type Rejection = CtxError;
    fn from_request_parts<'life0, 'life1, 'async_trait>(
        parts: &'life0 mut axum::http::request::Parts,
        _state: &'life1 S,
    ) -> core::pin::Pin<
        Box<dyn core::future::Future<Output = CtxResult<Self>> + core::marker::Send + 'async_trait>,
    >
    where
        'life0: 'async_trait,
        'life1: 'async_trait,
        Self: 'async_trait,
    {
        Box::pin(async {
            // println!(
            //     "->> {:<12} - Ctx::from_request_parts - extract Ctx from extension",
            //     "EXTRACTOR"
            // );
            parts.extensions.get::<Ctx>().cloned().ok_or(CtxError {
                req_id: Uuid::new_v4(),
                error: AppError::AuthFailCtxNotInRequestExt,
                is_htmx: false,
            })
        })
    }
}
