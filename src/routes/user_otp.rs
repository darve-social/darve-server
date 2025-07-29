use crate::{
    middleware::{
        auth_with_login_access::AuthWithLoginAccess, auth_with_otp_access::AuthWithOtpAccess,
    },
    utils::totp::{Totp, TotpResposne},
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
}

async fn otp_disable(
    auth_data: AuthWithLoginAccess,
    State(state): State<Arc<CtxState>>,
) -> CtxResult<()> {
    let local_user_db_service = LocalUserDbService {
        db: &state.db.client,
        ctx: &auth_data.ctx,
    };

    let mut user = local_user_db_service
        .get_by_id(&auth_data.user_thing_id())
        .await?;

    user.is_otp_enabled = false;
    user.otp_secret = None;
    local_user_db_service.update(user).await?;

    Ok(())
}

async fn otp_enable(
    auth_data: AuthWithLoginAccess,
    State(state): State<Arc<CtxState>>,
) -> CtxResult<Json<TotpResposne>> {
    let local_user_db_service = LocalUserDbService {
        db: &state.db.client,
        ctx: &auth_data.ctx,
    };

    let user_id = auth_data.user_thing_id();
    let mut user = local_user_db_service.get_by_id(&user_id).await?;
    let totp = Totp::new(&user_id, user.otp_secret.clone());
    let res = totp.generate();

    user.is_otp_enabled = true;
    user.otp_secret = Some(res.secret.clone());
    local_user_db_service.update(user).await?;

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

    let user = local_user_db_service.get_by_id(&user_id).await?;

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

    Ok((StatusCode::OK, Json(json!({"token": token, "user": user }))).into_response())
}
