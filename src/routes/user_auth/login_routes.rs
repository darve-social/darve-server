use std::collections::HashMap;
use std::sync::Arc;

use askama_axum::Template;
use axum::extract::Query;
use axum::response::{IntoResponse, Redirect, Response};
use axum::routing::get;
use axum::Router;
use serde::{Deserialize, Serialize};
use tower_cookies::{Cookie, Cookies};
use validator::Validate;

use middleware::mw_ctx::JWT_KEY;
use middleware::{ctx::Ctx, error::CtxResult};
use utils::askama_filter_util::filters;
use utils::template_utils::ProfileFormPage;

use crate::middleware::mw_ctx::CtxState;
use crate::{middleware, utils};

pub fn routes() -> Router<Arc<CtxState>> {
    Router::new()
        .route("/login", get(login_form))
        .route("/logout", get(logout_page))
}

#[derive(Debug, Deserialize, Serialize, Validate)]
pub struct LoginInput {
    pub username: String,
    pub password: String,
    pub next: Option<String>,
}

#[derive(Template, Serialize, Debug)]
#[template(path = "nera2/login_form.html")]
struct LoginForm {
    username: Option<String>,
    password: Option<String>,
    next: Option<String>,
    loggedin: bool,
}

#[derive(Template, Serialize, Debug)]
#[template(path = "nera2/logout_content.html")]
struct LogoutContent {
    next: Option<String>,
}

async fn login_form(
    ctx: Ctx,
    Query(mut qry): Query<HashMap<String, String>>,
) -> CtxResult<Response> {
    let next = qry.remove("next");
    if next.is_some() && ctx.user_id().is_ok() {
        return Ok(Redirect::temporary(next.unwrap().as_str()).into_response());
    }

    Ok(ProfileFormPage::new(
        Box::new(LoginForm {
            username: qry.remove("u"),
            password: qry.remove("p"),
            next,
            loggedin: ctx.user_id().is_ok(),
        }),
        None,
        None,
        None,
    )
    .into_response())
}

async fn logout_page(
    cookies: Cookies,
    ctx: Ctx,
    Query(mut qry): Query<HashMap<String, String>>,
) -> CtxResult<Response> {
    cookies.remove(Cookie::new(JWT_KEY, ""));

    let next = qry.remove("next");
    if next.is_some() && ctx.user_id().is_ok() {
        return Ok(Redirect::temporary(next.unwrap().as_str()).into_response());
    }

    Ok(ProfileFormPage::new(
        Box::new(LogoutContent {
            next: qry.remove("next"),
        }),
        None,
        None,
        None,
    )
    .into_response())
}
