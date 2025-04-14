use std::collections::HashMap;

use askama::Template;
use axum::extract::{Query, State};
use axum::response::{IntoResponse, Redirect, Response};
use axum::routing::get;
use axum::{routing::post, Router};
use once_cell::sync::Lazy;
use regex::Regex;
use serde::{Deserialize, Serialize};
use tower_cookies::Cookies;
use validator::Validate;

use crate::entity::authentication_entity::AuthType;
use crate::entity::local_user_entity::{LocalUser, LocalUserDbService};
use crate::routes::login_routes::{login, LoginInput};
use crate::utils::askama_filter_util::filters;
use crate::utils::template_utils::ProfileFormPage;
use sb_middleware::db::Db;
use sb_middleware::error::{AppResult, CtxResult};
use sb_middleware::mw_ctx::CtxState;
use sb_middleware::utils::db_utils::UsernameIdent;
use sb_middleware::utils::extractor_utils::JsonOrFormValidated;
use sb_middleware::utils::request_utils::CreatedResponse;
use sb_middleware::{ctx::Ctx, error::AppError, error::CtxError};

pub fn routes(state: CtxState) -> Router {
    Router::new()
        .route("/register", get(display_register_page))
        .route("/api/register", post(api_register))
        .with_state(state)
}

static USERNAME_REGEX: Lazy<Regex> = Lazy::new(|| Regex::new(r"^[A-Za-z0-9\_]{6,}$").unwrap());
pub fn validate_username(u: &String) -> AppResult<()> {
    if USERNAME_REGEX.is_match(u) {
        Ok(())
    }else { Err(AppError::Generic {description: "Username not valid".to_string()}) }
}

#[derive(Debug, Deserialize, Serialize, Validate)]
pub struct RegisterInput {
    #[validate(regex(
        path = * USERNAME_REGEX, message = "Letters, numbers and '_'. Minimum 6 characters."
    ))]
    pub username: String,
    #[validate(length(min = 6, message = "Min 6 characters"))]
    pub password: String,
    #[validate(length(min = 6, message = "Min 6 characters"))]
    pub password1: String,
    // TODO validate password
    // #[serde(skip_serializing_if = "Option::is_none")]
    // pub(crate) auth_type: Option<AuthType>,
    #[validate(email)]
    pub email: Option<String>,
    pub bio: Option<String>,
    #[validate(length(min = 6, message = "Min 1 character"))]
    pub full_name: Option<String>,
    #[validate(length(min = 6, message = "Min 6 characters"))]
    pub image_uri: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next: Option<String>,
}

impl RegisterInput {
    pub fn passwords_valid_auth_type(&self, ctx: &Ctx) -> CtxResult<AuthType> {
        if self.password != self.password1 {
            return Err(ctx.to_ctx_error(AppError::Generic {
                description: "Passwords must match".to_string(),
            }));
        }

        if self.password1.len() < 6 {
            return Err(ctx.to_ctx_error(AppError::Generic {
                description: "Password minimum 6 characters".to_string(),
            }));
        }

        Ok(AuthType::PASSWORD(Some(self.password.clone())))
    }
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

async fn api_register(
    State(ctx_state): State<CtxState>,
    cookies: Cookies,
    ctx: Ctx,
    // HxRequest(is_hx): HxRequest,
    JsonOrFormValidated(data): JsonOrFormValidated<RegisterInput>,
) -> CtxResult<Response> {
    println!("->> {:<12} - api_register", "HANDLER");

    // let JsonOrFormValidated(data)= payload;
    let _reg = register_user(&ctx_state._db, &ctx, &data).await?; //.map(|r|ctx.to_htmx_or_json(r))?;//.into_response();
                                                                  // let mut next = data.next.unwrap_or("".to_string());
                                                                  /*if next.len()<1{
                                                                      next = format!("/login?u={}", data.username);
                                                                  }*/
    // registered.headers_mut().insert(HX_REDIRECT, next.parse().unwrap());
    // Ok(registered)
    login(
        State(ctx_state),
        cookies,
        ctx,
        JsonOrFormValidated(LoginInput {
            username: data.username,
            password: data.password,
            next: data.next,
        }),
    )
    .await
}

pub async fn register_user(
    _db: &Db,
    ctx: &Ctx,
    payload: &RegisterInput,
) -> CtxResult<CreatedResponse> {
    let user_db_service = &LocalUserDbService {
        db: &_db,
        ctx: &ctx,
    };

    let auth_type = payload.passwords_valid_auth_type(ctx)?;

    let exists = user_db_service
        .exists(UsernameIdent(payload.username.clone()).into())
        .await?;
    // dbg!(&exists);
    if exists.is_none() {
        let created_id = user_db_service
            .create(
                LocalUser {
                    id: None,
                    username: payload.username.clone(),
                    full_name: payload.full_name.clone(),
                    birth_date: None,
                    phone: None,
                    email: payload.email.clone(),
                    bio: payload.bio.clone(),
                    social_links: None,
                    image_uri: payload.image_uri.clone(),
                },
                auth_type,
            )
            .await?;
        return Ok(CreatedResponse {
            success: true,
            id: created_id,
            uri: None,
        });
    } else if let AuthType::PASSWORD(_pass) = &auth_type {
        // TODO get jwt user, check if jwt.username==username and if no password auth add new auth
    }
    return Err(CtxError {
        error: AppError::RegisterFail,
        req_id: ctx.req_id(),
        is_htmx: ctx.is_htmx,
    });
}
