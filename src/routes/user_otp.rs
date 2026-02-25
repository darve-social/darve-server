use crate::{
    entities::user_auth::{authentication_entity::AuthType, local_user_entity::UpdateUser},
    middleware::{auth_with_otp_access::AuthWithOtpAccess, bearer_auth::BearerAuth},
    models::view::user::LoggedUserView,
    utils::totp::{Totp, TotpResponse},
};
use axum::{
    extract::State,
    response::{IntoResponse, Response},
    routing::post,
    Json, Router,
};
use reqwest::StatusCode;
use serde::Deserialize;
use serde_json::json;
use std::sync::Arc;

use crate::{
    entities::user_auth::local_user_entity::LocalUserDbService,
    middleware::{
        ctx::Ctx,
        error::{AppError, CtxResult},
        mw_ctx::CtxState,
    },
};

pub fn routes() -> Router<Arc<CtxState>> {
    Router::new()
        .route("/api/users/current/otp/enable", post(otp_enable))
        .route("/api/users/current/otp/disable", post(otp_disable))
        .route("/api/users/current/otp/validate", post(otp_validate))
        .route(
            "/api/users/current/otp/verification",
            post(otp_verification),
        )
}

async fn otp_verification(
    auth_data: BearerAuth,
    State(state): State<Arc<CtxState>>,
    Json(data): Json<OtpVerificationData>,
) -> CtxResult<Json<bool>> {
    let user_id = auth_data.user_thing_id();
    let local_user_db_service = LocalUserDbService {
        db: &state.db.client,
        ctx: &Ctx::new(Ok(auth_data.user_id.clone()), false),
    };

    let user = local_user_db_service.get_by_id(&user_id).await?;

    if user.otp_secret.is_none() || user.is_otp_enabled {
        return Err(AppError::Forbidden.into());
    }

    let totp = Totp::new(&user_id, user.otp_secret.clone());
    if !totp.is_valid(&data.token) {
        return Err(AppError::Generic {
            description: "Token is invalid".to_string(),
        }
        .into());
    };

    let update_user = UpdateUser {
        bio: None,
        birth_date: None,
        full_name: None,
        image_uri: None,
        is_otp_enabled: Some(true),
        otp_secret: None,
        phone: None,
        social_links: None,
        username: None,
    };
    local_user_db_service.update(&user_id, update_user).await?;
    Ok(Json(true))
}

async fn otp_disable(auth_data: BearerAuth, State(state): State<Arc<CtxState>>) -> CtxResult<()> {
    let user_id = auth_data.user_thing_id();
    let local_user_db_service = LocalUserDbService {
        db: &state.db.client,
        ctx: &auth_data.ctx,
    };

    let _user = local_user_db_service
        .get_by_id(&auth_data.user_thing_id())
        .await?;

    let update_user = UpdateUser {
        bio: None,
        birth_date: None,
        full_name: None,
        image_uri: None,
        is_otp_enabled: Some(false),
        otp_secret: Some(None),
        phone: None,
        social_links: None,
        username: None,
    };
    local_user_db_service.update(&user_id, update_user).await?;

    Ok(())
}

async fn otp_enable(
    auth_data: BearerAuth,
    State(state): State<Arc<CtxState>>,
) -> CtxResult<Json<TotpResponse>> {
    let local_user_db_service = LocalUserDbService {
        db: &state.db.client,
        ctx: &auth_data.ctx,
    };

    let user_id = auth_data.user_thing_id();
    let user = local_user_db_service.get_by_id(&user_id).await?;
    let totp = Totp::new(&user_id, user.otp_secret.clone());
    let res = totp.generate();

    let update_user = UpdateUser {
        bio: None,
        birth_date: None,
        full_name: None,
        image_uri: None,
        is_otp_enabled: None,
        otp_secret: Some(Some(res.secret.clone())),
        phone: None,
        social_links: None,
        username: None,
    };
    local_user_db_service.update(&user_id, update_user).await?;
    Ok(Json(res))
}

#[derive(Debug, Deserialize)]
struct OtpVerificationData {
    pub token: String,
}

async fn otp_validate(
    auth_data: AuthWithOtpAccess,
    State(state): State<Arc<CtxState>>,
    Json(data): Json<OtpVerificationData>,
) -> CtxResult<Response> {
    let user_id = auth_data.user_thing_id();
    let local_user_db_service = LocalUserDbService {
        db: &state.db.client,
        ctx: &Ctx::new(Ok(auth_data.user_id.clone()), false),
    };

    let (user, auth) = local_user_db_service
        .get_by_id_with_auth(&user_id, AuthType::PASSWORD)
        .await?;

    if !user.is_otp_enabled {
        return Err(AppError::Forbidden.into());
    }

    let totp = Totp::new(&user_id, user.otp_secret.clone());

    if !totp.is_valid(&data.token) {
        return Err(AppError::Forbidden.into());
    }

    let token = state
        .jwt
        .create_by_login(&user.id.as_ref().unwrap().to_raw())
        .map_err(|e| AppError::Generic {
            description: e.to_string(),
        })?;

    Ok((
        StatusCode::OK,
        Json(json!({"token": token, "user": LoggedUserView::from((user, auth.is_some())) })),
    )
        .into_response())
}
