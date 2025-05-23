use std::collections::HashMap;

use askama_axum::Template;
use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Redirect, Response};
use axum::routing::get;
use axum::{routing::post, Json, Router};
use axum_htmx::HX_REDIRECT;
use serde::{Deserialize, Serialize};
use tower_cookies::{Cookie, Cookies};
use validator::Validate;

use authentication_entity::{AuthType, AuthenticationDbService};
use local_user_entity::LocalUserDbService;
use middleware::mw_ctx::{CtxState, JWT_KEY};
use middleware::utils::cookie_utils;
use middleware::utils::db_utils::UsernameIdent;
use middleware::utils::extractor_utils::JsonOrFormValidated;
use middleware::{ctx::Ctx, error::AppError, error::CtxError, error::CtxResult};
use utils::askama_filter_util::filters;
use utils::template_utils::ProfileFormPage;

use crate::entities::user_auth::{authentication_entity, local_user_entity};
use crate::{middleware, utils};

pub fn routes(state: CtxState) -> Router {
    Router::new()
        .route("/api/login", post(login))
        .route("/login", get(login_form))
        .route("/logout", get(logout_page))
        .with_state(state)
}

#[derive(Debug, Deserialize, Serialize, Validate)]
pub struct LoginInput {
    pub username: String,
    pub password: String,
    pub next: Option<String>,
}

#[derive(Debug, Serialize)]
struct LoginSuccess {
    id: String,
    username: String,
    full_name: Option<String>,
    image_uri: Option<String>,
    bio: Option<String>,
    email: Option<String>,
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
    State(CtxState { _db, .. }): State<CtxState>,
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
    State(CtxState { _db, .. }): State<CtxState>,
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

pub async fn login(
    State(CtxState {
        _db,
        key_enc,
        jwt_duration,
        ..
    }): State<CtxState>,
    cookies: Cookies,
    ctx: Ctx,
    // domainIdent: HostDomainId,
    // HxRequest(is_htmx): HxRequest,
    JsonOrFormValidated(payload): JsonOrFormValidated<LoginInput>,
) -> CtxResult<Response> {
    let local_user_db_service = LocalUserDbService {
        ctx: &ctx,
        db: &_db,
    };

    let user = local_user_db_service
        .get(UsernameIdent(payload.username.clone()).into())
        .await?;

    let user_id = if payload.password.len() > 0 {
        let pass = payload.password.clone();
        AuthenticationDbService {
            ctx: &ctx,
            db: &_db,
        }
        .authenticate(
            &ctx,
            AuthType::PASSWORD(Some(pass), user.id.clone()),
        )
        .await?
    } else {
        return Err(CtxError {
            error: AppError::AuthenticationFail,
            req_id: ctx.req_id(),
            is_htmx: ctx.is_htmx,
        });
    };

    cookie_utils::issue_login_jwt(
        &key_enc,
        cookies,
        user.id.map(|v| v.to_raw()).clone(),
        jwt_duration,
    );
    let mut res = (
        StatusCode::OK,
        Json(LoginSuccess {
            id: user_id,
            username: payload.username.clone(),
            email: user.email.clone(),
            full_name: user.full_name.clone(),
            bio: user.bio.clone(),
            image_uri: user.image_uri.clone(),
        }),
    )
        .into_response();
    let mut next = payload.next.unwrap_or("".to_string());
    if next.len() < 1 {
        next = "/community".to_string();
    }
    res.headers_mut().insert(HX_REDIRECT, next.parse().unwrap());

    Ok(res)
}
