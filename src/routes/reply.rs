use std::sync::Arc;

use crate::access::post::PostAccess;
use crate::database::table_names::REPLY_TABLE_NAME;
use crate::entities::community::post_entity::PostDbService;
use crate::entities::user_auth::local_user_entity;
use crate::interfaces::repositories::like::LikesRepositoryInterface;
use crate::middleware;
use crate::middleware::auth_with_login_access::AuthWithLoginAccess;
use crate::middleware::error::AppError;
use crate::middleware::utils::db_utils::Pagination;
use crate::middleware::utils::extractor_utils::JsonOrFormValidated;
use crate::middleware::utils::string_utils::get_str_thing;
use crate::models::view::access::PostAccessView;
use crate::models::view::reply::ReplyView;
use crate::models::view::user::UserView;
use crate::services::notification_service::NotificationService;
use crate::services::post_service::PostLikeData;
use crate::utils::validate_utils::trim_string;
use axum::extract::{Path, Query, State};
use axum::routing::{delete, get, post};
use axum::{Json, Router};
use local_user_entity::LocalUserDbService;
use middleware::error::CtxResult;
use middleware::mw_ctx::CtxState;
use serde::{Deserialize, Serialize};
use validator::Validate;

pub fn routes() -> Router<Arc<CtxState>> {
    Router::new()
        .route("/api/replies/{reply_id}/unlike", delete(unlike))
        .route("/api/comments/{comment_id}/replies", post(create_reply))
        .route("/api/comments/{comment_id}/replies", get(get_replies))
        .route("/api/replies/{reply_id}/like", post(like))
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

    let mut reply = ctx_state
        .db
        .replies
        .get_by_id(&reply_thing.id.to_raw())
        .await?;

    if reply.belongs_to.tb == REPLY_TABLE_NAME {
        reply = ctx_state
            .db
            .replies
            .get_by_id(&reply.belongs_to.id.to_raw())
            .await?;
    }

    let post_db_service = PostDbService {
        db: &ctx_state.db.client,
        ctx: &auth_data.ctx,
    };

    let post = post_db_service
        .get_view_by_id::<PostAccessView>(&reply.belongs_to.to_raw(), None)
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

    let n_service = NotificationService::new(
        &ctx_state.db.client,
        &auth_data.ctx,
        &ctx_state.event_sender,
        &ctx_state.db.user_notifications,
    );

    n_service.on_reply_like(&user, &post).await?;

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

    let mut reply = ctx_state
        .db
        .replies
        .get_by_id(&reply_thing.id.to_raw())
        .await?;

    if reply.belongs_to.tb == REPLY_TABLE_NAME {
        reply = ctx_state
            .db
            .replies
            .get_by_id(&reply.belongs_to.id.to_raw())
            .await?;
    }

    let post_db_service = PostDbService {
        db: &ctx_state.db.client,
        ctx: &auth_data.ctx,
    };

    let post = post_db_service
        .get_view_by_id::<PostAccessView>(&reply.belongs_to.to_raw(), None)
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

#[derive(Deserialize, Serialize, Validate)]
pub struct ReplyInput {
    #[serde(deserialize_with = "trim_string")]
    #[validate(length(min = 1, message = "Content can not be empty"))]
    pub content: String,
}

async fn create_reply(
    State(state): State<Arc<CtxState>>,
    auth_data: AuthWithLoginAccess,
    Path(comment_id): Path<String>,
    JsonOrFormValidated(reply_input): JsonOrFormValidated<ReplyInput>,
) -> CtxResult<Json<ReplyView>> {
    let user = LocalUserDbService {
        db: &state.db.client,
        ctx: &auth_data.ctx,
    }
    .get_by_id(&auth_data.user_thing_id())
    .await?;
    let comment_thing = get_str_thing(&comment_id)?;
    let comment = state
        .db
        .replies
        .get_by_id(&comment_thing.id.to_raw())
        .await?;

    let post_db_service = PostDbService {
        db: &state.db.client,
        ctx: &auth_data.ctx,
    };
    let post = post_db_service
        .get_view_by_id::<PostAccessView>(&comment.belongs_to.to_raw(), None)
        .await?;

    if !PostAccess::new(&post).can_create_reply_for_reply(&user) {
        return Err(AppError::Forbidden.into());
    }

    let reply = state
        .db
        .replies
        .create(
            comment.id,
            user.id.as_ref().unwrap().id.to_raw().as_ref(),
            &reply_input.content,
        )
        .await?;

    let reply_view = ReplyView {
        id: reply.id,
        user: UserView::from(user),
        likes_nr: reply.likes_nr,
        content: reply.content,
        created_at: reply.created_at,
        updated_at: reply.updated_at,
        liked_by: None,
        replies_nr: reply.replies_nr,
    };

    Ok(Json(reply_view))
}

#[derive(Debug, Deserialize)]
pub struct GetRepliesQuery {
    pub start: Option<u32>,
    pub count: Option<u16>,
}

async fn get_replies(
    State(state): State<Arc<CtxState>>,
    auth_data: AuthWithLoginAccess,
    Path(comment_id): Path<String>,
    Query(query): Query<GetRepliesQuery>,
) -> CtxResult<Json<Vec<ReplyView>>> {
    let user = LocalUserDbService {
        db: &state.db.client,
        ctx: &auth_data.ctx,
    }
    .get_by_id(&auth_data.user_thing_id())
    .await?;

    let comment_thing = get_str_thing(&comment_id)?;
    let comment = state
        .db
        .replies
        .get_by_id(&comment_thing.id.to_raw())
        .await?;

    let post_db_service = PostDbService {
        db: &state.db.client,
        ctx: &auth_data.ctx,
    };
    let post = post_db_service
        .get_view_by_id::<PostAccessView>(&comment.belongs_to.to_raw(), None)
        .await?;

    if !PostAccess::new(&post).can_view(&user) {
        return Err(AppError::Forbidden.into());
    }

    let replies = state
        .db
        .replies
        .get(
            &auth_data.user_thing_id(),
            comment.id,
            Pagination {
                order_by: None,
                order_dir: None,
                count: query.count.unwrap_or(50),
                start: query.start.unwrap_or(0),
            },
        )
        .await?;
    Ok(Json(replies))
}
