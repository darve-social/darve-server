use std::sync::Arc;

use crate::access::post::PostAccess;
use crate::entities::community::post_entity::PostDbService;
use crate::entities::user_auth::local_user_entity;
use crate::interfaces::repositories::like::LikesRepositoryInterface;
use crate::middleware;
use crate::middleware::auth_with_login_access::AuthWithLoginAccess;
use crate::middleware::error::AppError;
use crate::middleware::utils::string_utils::get_str_thing;
use crate::models::view::access::PostAccessView;
use crate::services::post_service::PostLikeData;
use axum::extract::{Path, State};
use axum::routing::{delete, post};
use axum::{Json, Router};
use local_user_entity::LocalUserDbService;
use middleware::error::CtxResult;
use middleware::mw_ctx::CtxState;
use serde::{Deserialize, Serialize};
use validator::Validate;

pub fn routes() -> Router<Arc<CtxState>> {
    Router::new()
        .route("/api/replies/:reply_id/unlike", delete(unlike))
        .route("/api/replies/:reply_id/like", post(like))
}

#[derive(Debug, Serialize, Deserialize)]
pub struct LikeResponse {
    pub likes_count: u32,
}

async fn like(
    auth_data: AuthWithLoginAccess,
    Path(reply_id): Path<String>,
    State(ctx_state): State<Arc<CtxState>>,
    Json(body): Json<PostLikeData>,
) -> CtxResult<Json<LikeResponse>> {
    body.validate()?;
    let user_db_service = LocalUserDbService {
        db: &ctx_state.db.client,
        ctx: &auth_data.ctx,
    };
    let user = user_db_service.get_ctx_user().await?;

    let reply_thing = get_str_thing(&reply_id)?;

    let reply = ctx_state
        .db
        .replies
        .get_by_id(&reply_thing.id.to_raw())
        .await?;

    let post_db_service = PostDbService {
        db: &ctx_state.db.client,
        ctx: &auth_data.ctx,
    };

    let post = post_db_service
        .get_view_by_id::<PostAccessView>(&reply.belongs_to.to_raw())
        .await?;

    let likes = body.count.unwrap_or(1);
    let by_credits = body.count.is_some();

    if !PostAccess::new(&post).can_like(&user) {
        return Err(AppError::Forbidden.into());
    }

    if by_credits && user.credits < likes as u64 {
        return Err(AppError::Generic {
            description: "The user does not have enough credits".to_string(),
        }
        .into());
    }

    let count = ctx_state
        .db
        .likes
        .like(user.id.as_ref().unwrap().clone(), reply.id, likes)
        .await?;

    if by_credits {
        user_db_service
            .remove_credits(user.id.as_ref().unwrap().clone(), likes)
            .await?;
    }

    Ok(Json(LikeResponse { likes_count: count }))
}

async fn unlike(
    auth_data: AuthWithLoginAccess,
    Path(reply_id): Path<String>,
    State(ctx_state): State<Arc<CtxState>>,
) -> CtxResult<Json<LikeResponse>> {
    let user_id = auth_data.user_thing_id();
    let user = LocalUserDbService {
        db: &ctx_state.db.client,
        ctx: &auth_data.ctx,
    }
    .get_by_id(&user_id)
    .await?;
    let reply_thing = get_str_thing(&reply_id)?;

    let reply = ctx_state
        .db
        .replies
        .get_by_id(&reply_thing.id.to_raw())
        .await?;

    let post_db_service = PostDbService {
        db: &ctx_state.db.client,
        ctx: &auth_data.ctx,
    };

    let post = post_db_service
        .get_view_by_id::<PostAccessView>(&reply.belongs_to.to_raw())
        .await?;

    if !PostAccess::new(&post).can_like(&user) {
        return Err(AppError::Forbidden.into());
    }

    let count = ctx_state
        .db
        .likes
        .unlike(user.id.as_ref().unwrap().clone(), reply.id)
        .await?;

    Ok(Json(LikeResponse { likes_count: count }))
}
