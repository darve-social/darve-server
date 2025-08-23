use std::collections::HashMap;
use std::sync::Arc;

use askama::Template;
use axum::extract::Query;
use axum::response::{IntoResponse, Redirect, Response};
use axum::routing::get;
use axum::Router;
use serde::Serialize;

use middleware::ctx::Ctx;
use middleware::error::CtxResult;
use middleware::utils::request_utils::CreatedResponse;
use utils::askama_filter_util::filters;
use utils::template_utils::ProfileFormPage;
use validator::Validate;

use crate::middleware::mw_ctx::CtxState;
use crate::services::auth_service::{AuthRegisterInput, AuthService};

use crate::{middleware, utils};

pub fn routes() -> Router<Arc<CtxState>> {
    Router::new().route("/register", get(display_register_page))
}

#[derive(Template, Serialize, Debug)]
#[template(path = "nera2/register_form.html")]
struct RegisterForm {
    email: Option<String>,
    username: Option<String>,
    next: Option<String>,
    loggedin: bool,
}

pub async fn display_register_page(
    ctx: Ctx,
    Query(mut qry): Query<HashMap<String, String>>,
) -> CtxResult<Response> {
    let next = qry.remove("next");
    if next.is_some() && ctx.user_id().is_ok() {
        return Ok(Redirect::temporary(next.unwrap().as_str()).into_response());
    }

    Ok(ProfileFormPage::new(
        Box::new(RegisterForm {
            username: qry.remove("u"),
            email: None,
            next,
            loggedin: ctx.user_id().is_ok(),
        }),
        None,
        None,
        None,
    )
    .into_response())
}

pub async fn display_register_form(
    ctx: Ctx,
    Query(mut qry): Query<HashMap<String, String>>,
) -> CtxResult<Response> {
    let next = qry.remove("next");
    if next.is_some() && ctx.user_id().is_ok() {
        return Ok(Redirect::temporary(next.unwrap().as_str()).into_response());
    }

    Ok(RegisterForm {
        username: qry.remove("u"),
        email: None,
        next,
        loggedin: ctx.user_id().is_ok(),
    }
    .into_response())
}

pub async fn register_user(
    state: &Arc<CtxState>,
    ctx: &Ctx,
    payload: AuthRegisterInput,
) -> CtxResult<CreatedResponse> {
    payload.validate()?;

    let auth_service = AuthService::new(
        &state.db.client,
        &ctx,
        &state.jwt,
        &state.email_sender,
        state.verification_code_ttl,
        &state.db.verification_code,
        &state.db.access,
    );

    let (_, user) = auth_service.register_password(payload).await?;
    Ok(CreatedResponse {
        success: true,
        id: user.id.unwrap().to_raw(),
        uri: None,
    })
}
