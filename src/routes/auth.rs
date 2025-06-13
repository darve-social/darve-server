use std::sync::Arc;

use axum::{
    extract::State,
    response::{IntoResponse, Response},
    routing::post,
    Json, Router,
};
use reqwest::StatusCode;
use serde::{Deserialize, Serialize};
use serde_json::json;
use tower_cookies::{Cookie, Cookies};

use crate::{
    entities::user_auth::{
        authentication_entity::AuthenticationDbService, local_user_entity::LocalUserDbService,
    },
    middleware::{
        ctx::Ctx,
        error::CtxResult,
        mw_ctx::{CtxState, JWT_KEY},
        utils::extractor_utils::JsonOrFormValidated,
    },
    services::{
        auth_service::{
            AuthLoginInput, AuthRegisterInput, AuthService, ForgotPasswordInput, ResetPasswordInput,
        },
        user_service::UserService,
    },
};

pub fn routes() -> Router<Arc<CtxState>> {
    Router::new()
        .route("/api/auth/sign_with_facebook", post(sign_by_fb))
        .route("/api/auth/sign_with_apple", post(sign_by_apple))
        .route("/api/auth/sign_with_google", post(sign_by_google))
        .route("/api/forgot_password", post(forgot_password))
        .route("/api/reset_password", post(reset_password))
        .route("/api/login", post(signin))
        .route("/api/register", post(signup))
}

#[derive(Debug, Deserialize, Serialize)]
struct SocialSignInput {
    token: String,
}
async fn sign_by_fb(
    State(state): State<Arc<CtxState>>,
    cookies: Cookies,
    ctx: Ctx,
    body: Json<SocialSignInput>,
) -> CtxResult<Response> {
    let auth_service = AuthService::new(
        &state.db.client,
        &ctx,
        &state.jwt,
        &state.email_sender,
        state.verification_code_ttl,
        &state.db.verification_code,
    );

    let (token, user) = auth_service.sign_by_facebook(&body.token).await?;

    cookies.add(
        Cookie::build((JWT_KEY, token))
            .path("/")
            .http_only(true)
            .into(),
    );

    Ok((StatusCode::OK, Json(json!(user))).into_response())
}

async fn sign_by_apple(
    State(state): State<Arc<CtxState>>,
    ctx: Ctx,
    cookies: Cookies,
    body: Json<SocialSignInput>,
) -> CtxResult<Response> {
    let auth_service = AuthService::new(
        &state.db.client,
        &ctx,
        &state.jwt,
        &state.email_sender,
        state.verification_code_ttl,
        &state.db.verification_code,
    );
    let (token, user) = auth_service
        .register_login_by_apple(&body.token, &state.apple_mobile_client_id)
        .await?;

    cookies.add(
        Cookie::build((JWT_KEY, token))
            // if not set, the path defaults to the path from which it was called - prohibiting gql on root if login is on /api
            .path("/")
            .http_only(true)
            .into(), //.finish(),
    );

    Ok((StatusCode::OK, Json(json!(user))).into_response())
}

async fn sign_by_google(
    State(state): State<Arc<CtxState>>,
    cookies: Cookies,
    ctx: Ctx,
    body: Json<SocialSignInput>,
) -> CtxResult<Response> {
    let auth_service = AuthService::new(
        &state.db.client,
        &ctx,
        &state.jwt,
        &state.email_sender,
        state.verification_code_ttl,
        &state.db.verification_code,
    );

    let (token, user) = auth_service
        .sign_by_google(&body.token, &state.google_client_id)
        .await?;

    cookies.add(
        Cookie::build((JWT_KEY, token))
            // if not set, the path defaults to the path from which it was called - prohibiting gql on root if login is on /api
            .path("/")
            .http_only(true)
            .into(), //.finish(),
    );

    Ok((StatusCode::OK, Json(json!(user))).into_response())
}

async fn signin(
    State(state): State<Arc<CtxState>>,
    cookies: Cookies,
    ctx: Ctx,
    JsonOrFormValidated(body): JsonOrFormValidated<AuthLoginInput>,
) -> CtxResult<Response> {
    let auth_service = AuthService::new(
        &state.db.client,
        &ctx,
        &state.jwt,
        &state.email_sender,
        state.verification_code_ttl,
        &state.db.verification_code,
    );

    let (token, user) = auth_service.login_password(body).await?;

    cookies.add(
        Cookie::build((JWT_KEY, token))
            // if not set, the path defaults to the path from which it was called - prohibiting gql on root if login is on /api
            .path("/")
            .http_only(true)
            .into(), //.finish(),
    );

    Ok((StatusCode::OK, Json(json!(user))).into_response())
}

async fn signup(
    State(state): State<Arc<CtxState>>,
    cookies: Cookies,
    ctx: Ctx,
    JsonOrFormValidated(body): JsonOrFormValidated<AuthRegisterInput>,
) -> CtxResult<Response> {
    let email = body.email.clone();
    let auth_service = AuthService::new(
        &state.db.client,
        &ctx,
        &state.jwt,
        &state.email_sender,
        state.verification_code_ttl,
        &state.db.verification_code,
    );

    let (token, user) = auth_service.register_password(body).await?;

    if let Some(email) = email {
        let user_service = UserService::new(
            LocalUserDbService {
                db: &state.db.client,
                ctx: &ctx,
            },
            &state.email_sender,
            state.verification_code_ttl,
            AuthenticationDbService {
                db: &state.db.client,
                ctx: &ctx,
            },
            &state.db.verification_code,
        );

        user_service
            .start_email_verification(user.id.as_ref().unwrap().to_raw().as_str(), &email)
            .await?;
    }

    cookies.add(
        Cookie::build((JWT_KEY, token))
            // if not set, the path defaults to the path from which it was called - prohibiting gql on root if login is on /api
            .path("/")
            .http_only(true)
            .into(), //.finish(),
    );

    Ok((StatusCode::OK, Json(json!(user))).into_response())
}

async fn forgot_password(
    State(state): State<Arc<CtxState>>,
    ctx: Ctx,
    Json(body): Json<ForgotPasswordInput>,
) -> CtxResult<Response> {
    let auth_service = AuthService::new(
        &state.db.client,
        &ctx,
        &state.jwt,
        &state.email_sender,
        state.verification_code_ttl,
        &state.db.verification_code,
    );

    let _ = auth_service.forgot_password(body).await?;
    Ok((StatusCode::OK).into_response())
}

async fn reset_password(
    State(state): State<Arc<CtxState>>,
    ctx: Ctx,
    Json(body): Json<ResetPasswordInput>,
) -> CtxResult<Response> {
    let auth_service = AuthService::new(
        &state.db.client,
        &ctx,
        &state.jwt,
        &state.email_sender,
        state.verification_code_ttl,
        &state.db.verification_code,
    );

    let _ = auth_service.reset_password(body).await?;
    Ok((StatusCode::OK).into_response())
}
