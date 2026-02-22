use std::sync::Arc;

use axum::{
    extract::{DefaultBodyLimit, State},
    response::{IntoResponse, Response},
    routing::post,
    Json, Router,
};
use axum_typed_multipart::TypedMultipart;
use reqwest::StatusCode;
use serde::{Deserialize, Serialize};
use serde_json::json;
use tower_cookies::{Cookie, Cookies};

use crate::{
    middleware::{
        ctx::Ctx,
        error::CtxResult,
        mw_ctx::{CtxState, JWT_KEY},
        utils::extractor_utils::JsonOrFormValidated,
    },
    models::view::user::LoggedUserView,
    services::auth_service::{
        AuthLoginInput, AuthRegisterInput, AuthService, ForgotPasswordInput, ResetPasswordInput,
    },
};

pub fn routes() -> Router<Arc<CtxState>> {
    Router::new()
        .route("/api/auth/sign_with_facebook", post(sign_by_fb))
        .route("/api/auth/sign_with_apple", post(sign_by_apple))
        .route("/api/auth/sign_with_google", post(sign_by_google))
        .route("/api/forgot_password/start", post(forgot_password_start))
        .route(
            "/api/forgot_password/confirm",
            post(forgot_password_confirm),
        )
        .route("/api/login", post(signin))
        .route(
            "/api/register",
            post(signup).layer(DefaultBodyLimit::max(1024 * 1024 * 8)),
        )
}

#[derive(Debug, Deserialize, Serialize)]
struct SocialSignInput {
    token: String,
}
async fn sign_by_fb(
    State(state): State<Arc<CtxState>>,
    ctx: Ctx,
    body: Json<SocialSignInput>,
) -> CtxResult<Response> {
    let auth_service = AuthService::new(
        &state.db.client,
        &ctx,
        &state.jwt,
        state.email_sender.clone(),
        state.verification_code_ttl,
        &state.db.verification_code,
        &state.db.access,
        state.file_storage.clone(),
    );

    let (token, user, has_password) = auth_service.sign_by_facebook(&body.token).await?;

    Ok((
        StatusCode::OK,
        Json(json!({"token": token, "user": LoggedUserView::from((user, has_password)) })),
    )
        .into_response())
}

async fn sign_by_apple(
    State(state): State<Arc<CtxState>>,
    ctx: Ctx,
    body: Json<SocialSignInput>,
) -> CtxResult<Response> {
    let auth_service = AuthService::new(
        &state.db.client,
        &ctx,
        &state.jwt,
        state.email_sender.clone(),
        state.verification_code_ttl,
        &state.db.verification_code,
        &state.db.access,
        state.file_storage.clone(),
    );
    let (token, user, has_password) = auth_service
        .register_login_by_apple(&body.token, &state.apple_mobile_client_id)
        .await?;

    Ok((
        StatusCode::OK,
        Json(json!({"token": token, "user": LoggedUserView::from((user, has_password)) })),
    )
        .into_response())
}

async fn sign_by_google(
    State(state): State<Arc<CtxState>>,
    ctx: Ctx,
    body: Json<SocialSignInput>,
) -> CtxResult<Response> {
    let auth_service = AuthService::new(
        &state.db.client,
        &ctx,
        &state.jwt,
        state.email_sender.clone(),
        state.verification_code_ttl,
        &state.db.verification_code,
        &state.db.access,
        state.file_storage.clone(),
    );

    let (token, user, has_password) = auth_service
        .sign_by_google(
            &body.token,
            &vec![
                state.google_ios_client_id.as_str(),
                state.google_android_client_id.as_str(),
            ],
        )
        .await?;

    Ok((
        StatusCode::OK,
        Json(json!({"token": token, "user": LoggedUserView::from((user, has_password)) })),
    )
        .into_response())
}

async fn signin(
    State(state): State<Arc<CtxState>>,
    ctx: Ctx,
    cookies: Cookies,
    JsonOrFormValidated(body): JsonOrFormValidated<AuthLoginInput>,
) -> CtxResult<Response> {
    let auth_service = AuthService::new(
        &state.db.client,
        &ctx,
        &state.jwt,
        state.email_sender.clone(),
        state.verification_code_ttl,
        &state.db.verification_code,
        &state.db.access,
        state.file_storage.clone(),
    );

    let (token, user) = auth_service.login_password(body).await?;

    cookies.add(
        Cookie::build((JWT_KEY, token.clone()))
            // if not set, the path defaults to the path from which it was called - prohibiting gql on root if login is on /api
            .path("/")
            .http_only(true)
            .into(), //.finish(),
    );
    Ok((
        StatusCode::OK,
        Json(json!({"token": token, "user": LoggedUserView::from((user, true)) })),
    )
        .into_response())
}

async fn signup(
    State(state): State<Arc<CtxState>>,
    cookies: Cookies,
    ctx: Ctx,
    TypedMultipart(body): TypedMultipart<AuthRegisterInput>,
) -> CtxResult<Response> {
    let auth_service = AuthService::new(
        &state.db.client,
        &ctx,
        &state.jwt,
        state.email_sender.clone(),
        state.verification_code_ttl,
        &state.db.verification_code,
        &state.db.access,
        state.file_storage.clone(),
    );

    let (token, user) = auth_service.register_password(body, None).await?;

    cookies.add(
        Cookie::build((JWT_KEY, token.clone()))
            // if not set, the path defaults to the path from which it was called - prohibiting gql on root if login is on /api
            .path("/")
            .http_only(true)
            .into(), //.finish(),
    );

    Ok((
        StatusCode::OK,
        Json(json!({"token": token, "user": LoggedUserView::from((user, true)) })),
    )
        .into_response())
}

async fn forgot_password_start(
    State(state): State<Arc<CtxState>>,
    ctx: Ctx,
    Json(body): Json<ForgotPasswordInput>,
) -> CtxResult<Response> {
    let auth_service = AuthService::new(
        &state.db.client,
        &ctx,
        &state.jwt,
        state.email_sender.clone(),
        state.verification_code_ttl,
        &state.db.verification_code,
        &state.db.access,
        state.file_storage.clone(),
    );

    auth_service.forgot_password(body).await?;
    Ok((StatusCode::OK).into_response())
}

async fn forgot_password_confirm(
    State(state): State<Arc<CtxState>>,
    ctx: Ctx,
    Json(body): Json<ResetPasswordInput>,
) -> CtxResult<Response> {
    let auth_service = AuthService::new(
        &state.db.client,
        &ctx,
        &state.jwt,
        state.email_sender.clone(),
        state.verification_code_ttl,
        &state.db.verification_code,
        &state.db.access,
        state.file_storage.clone(),
    );

    let _ = auth_service.reset_password(body).await?;
    Ok((StatusCode::OK).into_response())
}
