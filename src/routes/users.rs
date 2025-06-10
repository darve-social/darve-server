use axum::{
    extract::State,
    response::{IntoResponse, Response},
    routing::{patch, post},
    Router,
};
use serde::Deserialize;
use validator::Validate;

use crate::{
    entities::user_auth::{
        authentication_entity::AuthenticationDbService, local_user_entity::LocalUserDbService,
    },
    middleware::{
        ctx::Ctx, error::CtxResult, mw_ctx::CtxState, utils::extractor_utils::JsonOrFormValidated,
    },
    services::user_service::UserService,
};

pub fn routes(state: CtxState) -> Router {
    Router::new()
        .route("/api/users/current/password", patch(reset_password))
        .route("/api/users/current/password", post(set_password))
        .with_state(state)
}

#[derive(Debug, Deserialize, Validate)]
struct SetPasswordInput {
    #[validate(length(min = 6, message = "Min 6 characters"))]
    password: String,
}

async fn set_password(
    State(state): State<CtxState>,
    ctx: Ctx,
    JsonOrFormValidated(data): JsonOrFormValidated<SetPasswordInput>,
) -> CtxResult<Response> {
    let user_id = ctx.user_id()?;

    let user_service = UserService::new(
        LocalUserDbService {
            db: &state._db,
            ctx: &ctx,
        },
        state.email_sender,
        state.verification_code_ttl,
        AuthenticationDbService {
            db: &state._db,
            ctx: &ctx,
        },
    );

    user_service.set_password(&user_id, &data.password).await?;

    Ok(().into_response())
}

#[derive(Debug, Deserialize, Validate)]
struct ResetPasswordInput {
    #[validate(length(min = 6, message = "Min 6 characters"))]
    old_password: String,
    #[validate(length(min = 6, message = "Min 6 characters"))]
    new_password: String,
}

async fn reset_password(
    State(state): State<CtxState>,
    ctx: Ctx,
    JsonOrFormValidated(data): JsonOrFormValidated<ResetPasswordInput>,
) -> CtxResult<Response> {
    let user_id = ctx.user_id()?;

    let user_service = UserService::new(
        LocalUserDbService {
            db: &state._db,
            ctx: &ctx,
        },
        state.email_sender,
        state.verification_code_ttl,
        AuthenticationDbService {
            db: &state._db,
            ctx: &ctx,
        },
    );

    user_service
        .update_password(&user_id, &data.new_password, &data.old_password)
        .await?;

    Ok("".into_response())
}
