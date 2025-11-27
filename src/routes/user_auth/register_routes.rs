use std::collections::HashMap;
use std::sync::Arc;

use askama::Template;
use axum::extract::Query;
use axum::response::{Html, IntoResponse, Redirect, Response};
use axum::routing::get;
use axum::Router;
use serde::Serialize;

use middleware::ctx::Ctx;
use middleware::error::CtxResult;
use utils::askama_filter_util::filters;
use utils::template_utils::ProfileFormPage;

use crate::middleware::mw_ctx::CtxState;
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

    let data = ProfileFormPage::new(
        Box::new(RegisterForm {
            username: qry.remove("u"),
            email: None,
            next,
            loggedin: ctx.user_id().is_ok(),
        }),
        None,
        None,
        None,
    );

    Ok(Html(data.render().unwrap()).into_response())
}

pub async fn display_register_form(
    ctx: Ctx,
    Query(mut qry): Query<HashMap<String, String>>,
) -> CtxResult<Response> {
    let next = qry.remove("next");
    if next.is_some() && ctx.user_id().is_ok() {
        return Ok(Redirect::temporary(next.unwrap().as_str()).into_response());
    }

    let data = RegisterForm {
        username: qry.remove("u"),
        email: None,
        next,
        loggedin: ctx.user_id().is_ok(),
    };

    Ok(Html(data.render().unwrap()).into_response())
}
