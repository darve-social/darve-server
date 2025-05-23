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
    middleware::{
        ctx::Ctx,
        error::CtxResult,
        mw_ctx::{CtxState, JWT_KEY},
    },
    services::auth_service::{AuthService, AuthLoginInput, AuthRegisterInput},
};

pub fn routes(state: CtxState) -> Router {
    Router::new()
        .route("/api/auth/sign_with_facebook", post(sign_by_fb))
        .route("/api/auth/sign_with_apple", post(sign_by_apple))
        .route("/api/auth/sign_with_google", post(sign_by_google))
        .route("/api/auth/signin", post(signin))
        .route("/api/auth/signup", post(signup))
        .with_state(state)
}

#[derive(Debug, Deserialize, Serialize)]
struct SocialSignInput {
    token: String,
}

async fn sign_by_fb(
    State(state): State<CtxState>,
    ctx: Ctx,
    cookies: Cookies,
    body: Json<SocialSignInput>,
) -> CtxResult<Response> {
    let auth_service = AuthService::new(&state._db, &ctx, state.jwt.clone());

    let (token, user) = auth_service.sign_by_facebook(&body.token).await?;

    cookies.add(
        Cookie::build((JWT_KEY, token))
            // if not set, the path defaults to the path from which it was called - prohibiting gql on root if login is on /api
            .path("/")
            .http_only(true)
            .into(), //.finish(),
    );

    Ok((StatusCode::OK, Json(json!(user))).into_response())
}

async fn sign_by_apple(
    State(state): State<CtxState>,
    ctx: Ctx,
    cookies: Cookies,
    body: Json<SocialSignInput>,
) -> CtxResult<Response> {
    let auth_service = AuthService::new(&state._db, &ctx, state.jwt.clone());

    let (token, user) = auth_service
        .register_login_by_apple(&body.token, &state.mobile_client_id)
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
    State(state): State<CtxState>,
    ctx: Ctx,
    cookies: Cookies,
    body: Json<SocialSignInput>,
) -> CtxResult<Response> {
    let auth_service = AuthService::new(&state._db, &ctx, state.jwt.clone());

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
    State(state): State<CtxState>,
    ctx: Ctx,
    cookies: Cookies,
    Json(body): Json<AuthLoginInput>,
) -> CtxResult<Response> {
    let auth_service = AuthService::new(&state._db, &ctx, state.jwt.clone());

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
    State(state): State<CtxState>,
    ctx: Ctx,
    cookies: Cookies,
    Json(body): Json<AuthRegisterInput>,
) -> CtxResult<Response> {
    let auth_service = AuthService::new(&state._db, &ctx, state.jwt.clone());

    let (token, user) = auth_service.register_password(body).await?;

    cookies.add(
        Cookie::build((JWT_KEY, token))
            // if not set, the path defaults to the path from which it was called - prohibiting gql on root if login is on /api
            .path("/")
            .http_only(true)
            .into(), //.finish(),
    );

    Ok((StatusCode::OK, Json(json!(user))).into_response())
}
