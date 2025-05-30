use std::collections::HashMap;

use askama::Template;
use axum::extract::Query;
use axum::response::{IntoResponse, Redirect, Response};
use axum::routing::get;
use axum::Router;
use serde::Serialize;

use middleware::ctx::Ctx;
use middleware::error::CtxResult;
use middleware::mw_ctx::CtxState;
use middleware::utils::request_utils::CreatedResponse;
use utils::askama_filter_util::filters;
use utils::template_utils::ProfileFormPage;
use validator::Validate;

use crate::services::auth_service::{AuthRegisterInput, AuthService};
use crate::{middleware, utils};

pub fn routes(state: CtxState) -> Router {
    Router::new()
        .route("/register", get(display_register_page))
        .with_state(state)
}

#[derive(Template, Serialize, Debug)]
#[template(path = "nera2/register_form.html")]
struct RegisterForm {
    username: Option<String>,
    next: Option<String>,
    loggedin: bool,
}

pub async fn display_register_page(
    // State(CtxState { _db, .. }): State<CtxState>,
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
        next,
        loggedin: ctx.user_id().is_ok(),
    }
    .into_response())
}

pub async fn register_user(
    state: &CtxState,
    ctx: &Ctx,
    payload: AuthRegisterInput,
) -> CtxResult<CreatedResponse> {
    payload.validate()?;

    let auth_service = AuthService::new(&state._db, ctx, state.jwt.clone());
    let (_, user) = auth_service.register_password(payload).await?;
    Ok(CreatedResponse {
        success: true,
        id: user.id.unwrap().to_raw(),
        uri: None,
    })
}
