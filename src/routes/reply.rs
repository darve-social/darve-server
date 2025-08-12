use std::sync::Arc;

use crate::entities::user_auth::local_user_entity;
use crate::interfaces::repositories::like::LikesRepositoryInterface;
use crate::middleware;
use crate::middleware::auth_with_login_access::AuthWithLoginAccess;
use crate::middleware::utils::string_utils::get_str_thing;
use axum::extract::{Path, State};
use axum::routing::{delete, post};
use axum::{Json, Router};
use local_user_entity::LocalUserDbService;
use middleware::error::CtxResult;
use middleware::mw_ctx::CtxState;
use serde::{Deserialize, Serialize};

pub fn routes() -> Router<Arc<CtxState>> {
    Router::new()
        .route("/api/replies/:reply_id/unlike", delete(unlike))
        .route("/api/replies/:reply_id/like", post(like))
}

#[derive(Debug, Serialize, Deserialize)]
pub struct LikeData {
    pub count: Option<u16>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct LikeResponse {
    pub likes_count: u32,
}

async fn like(
    auth_data: AuthWithLoginAccess,
    Path(reply_id): Path<String>,
    State(ctx_state): State<Arc<CtxState>>,
    Json(body): Json<LikeData>,
) -> CtxResult<Json<LikeResponse>> {
    let user_id = LocalUserDbService {
        db: &ctx_state.db.client,
        ctx: &auth_data.ctx,
    }
    .get_ctx_user_thing()
    .await?;
    let reply_thing = get_str_thing(&reply_id)?;
    let reply = ctx_state
        .db
        .replies
        .get_by_id(&reply_thing.id.to_raw())
        .await?;

    // TODO check access to the reply
    let count = ctx_state
        .db
        .likes
        .like(
            user_id,
            reply.id,
            body.count.unwrap_or(1),
        )
        .await?;

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

    // TODO check access to the reply

    let count = ctx_state
        .db
        .likes
        .unlike(user.id.as_ref().unwrap().clone(), reply.id)
        .await?;

    Ok(Json(LikeResponse { likes_count: count }))
}
