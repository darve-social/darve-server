use std::sync::Arc;

use axum::extract::{Path, Query, State};
use axum::routing::{delete, get, post};
use axum::{Json, Router};
use local_user_entity::LocalUserDbService;
use middleware::ctx::Ctx;
use middleware::error::CtxResult;
use middleware::mw_ctx::CtxState;
use post_entity::{Post, PostDbService};
use serde::{Deserialize, Serialize};
use validator::Validate;

use crate::access::post::PostAccess;
use crate::entities::community::post_entity::{self};
use crate::entities::task::task_request_entity::{TaskRequest, TaskRequestDbService};
use crate::entities::user_auth::local_user_entity;

use crate::middleware;
use crate::middleware::auth_with_login_access::AuthWithLoginAccess;
use crate::middleware::error::AppError;
use crate::middleware::utils::db_utils::{Pagination, QryOrder};
use crate::middleware::utils::extractor_utils::JsonOrFormValidated;
use crate::models::view::access::PostAccessView;
use crate::models::view::post::PostView;
use crate::models::view::reply::ReplyView;
use crate::models::view::task::TaskRequestView;
use crate::models::view::user::UserView;
use crate::services::notification_service::NotificationService;
use crate::services::post_service::{PostLikeData, PostService};
use crate::services::post_user_service::PostUserService;
use crate::services::task_service::{TaskRequestInput, TaskService};

pub fn routes() -> Router<Arc<CtxState>> {
    Router::new()
        .route("/api/posts", get(get_posts))
        .route("/api/posts/:post_id/tasks", post(create_task))
        .route("/api/posts/:post_id/tasks", get(get_post_tasks))
        .route("/api/posts/:post_id/like", post(like))
        .route("/api/posts/:post_id/unlike", delete(unlike))
        .route("/api/posts/:post_id/deliver", post(post_mark_as_deliver))
        .route("/api/posts/:post_id/read", post(post_mark_as_read))
        .route("/api/posts/:post_id/replies", post(create_reply))
        .route("/api/posts/:post_id/replies", get(get_replies))
        .route("/api/posts/:post_id/add_users", post(add_members))
        .route("/api/posts/:post_id/remove_users", post(remove_members))
        .route("/api/posts/:post_id/users", get(get_members))
}

#[derive(Debug, Deserialize)]
pub struct GetPostsQuery {
    pub tag: String,
    pub order_dir: Option<QryOrder>,
    pub start: Option<u32>,
    pub count: Option<u16>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GetPostsResponse {
    pub posts: Vec<Post>,
}

async fn create_task(
    auth_data: AuthWithLoginAccess,
    State(state): State<Arc<CtxState>>,
    Path(post_id): Path<String>,
    Json(body): Json<TaskRequestInput>,
) -> CtxResult<Json<TaskRequest>> {
    let task_service = TaskService::new(
        &state.db.client,
        &auth_data.ctx,
        &state.event_sender,
        &state.db.user_notifications,
        &state.db.task_donors,
        &state.db.task_participants,
        &state.db.access,
    );

    let task = task_service
        .create_for_post(&auth_data.user_thing_id(), &post_id, body)
        .await?;

    Ok(Json(task))
}

async fn get_post_tasks(
    auth_data: AuthWithLoginAccess,
    Path(post_id): Path<String>,
    State(state): State<Arc<CtxState>>,
) -> CtxResult<Json<Vec<TaskRequestView>>> {
    let user = LocalUserDbService {
        ctx: &auth_data.ctx,
        db: &state.db.client,
    }
    .get_by_id(&auth_data.user_thing_id())
    .await?;

    let post_db_service = PostDbService {
        ctx: &auth_data.ctx,
        db: &state.db.client,
    };

    let post = post_db_service
        .get_view_by_id::<PostAccessView>(&post_id)
        .await?;

    if !PostAccess::new(&post).can_view(&user) {
        return Err(AppError::Forbidden.into());
    }

    let task_db_service = TaskRequestDbService {
        ctx: &auth_data.ctx,
        db: &state.db.client,
    };

    let tasks = task_db_service
        .get_by_post(post.id, user.id.as_ref().unwrap().clone())
        .await?;

    Ok(Json(tasks))
}

async fn get_posts(
    Query(query): Query<GetPostsQuery>,
    State(state): State<Arc<CtxState>>,
    ctx: Ctx,
) -> CtxResult<Json<Vec<PostView>>> {
    let post_db_service = PostDbService {
        ctx: &ctx,
        db: &state.db.client,
    };
    let pagination = Pagination {
        order_by: Some("id".to_string()),
        order_dir: query.order_dir,
        count: query.count.unwrap_or(100),
        start: query.start.unwrap_or_default(),
    };
    let posts = post_db_service.get_by_tag(&query.tag, pagination).await;
    println!("{:?}", posts);

    Ok(Json(posts?))
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PostLikeResponse {
    pub likes_count: u32,
}

async fn like(
    auth_data: AuthWithLoginAccess,
    Path(post_id): Path<String>,
    State(ctx_state): State<Arc<CtxState>>,
    Json(body): Json<PostLikeData>,
) -> CtxResult<Json<PostLikeResponse>> {
    let user_id = auth_data.user_thing_id();
    let count = PostService::new(
        &ctx_state.db.client,
        &auth_data.ctx,
        &ctx_state.event_sender,
        &ctx_state.db.user_notifications,
        ctx_state.file_storage.clone(),
        &ctx_state.db.tags,
        &ctx_state.db.likes,
        &ctx_state.db.access,
    )
    .like(&post_id, &user_id, body)
    .await?;

    Ok(Json(PostLikeResponse { likes_count: count }))
}

async fn unlike(
    auth_data: AuthWithLoginAccess,
    Path(post_id): Path<String>,
    State(ctx_state): State<Arc<CtxState>>,
) -> CtxResult<Json<PostLikeResponse>> {
    let count = PostService::new(
        &ctx_state.db.client,
        &auth_data.ctx,
        &ctx_state.event_sender,
        &ctx_state.db.user_notifications,
        ctx_state.file_storage.clone(),
        &ctx_state.db.tags,
        &ctx_state.db.likes,
        &ctx_state.db.access,
    )
    .unlike(&post_id, &auth_data.user_thing_id())
    .await?;

    Ok(Json(PostLikeResponse { likes_count: count }))
}

#[derive(Debug, Deserialize)]
pub struct GetRepliesQuery {
    pub start: Option<u32>,
    pub count: Option<u16>,
}

async fn get_replies(
    State(state): State<Arc<CtxState>>,
    auth_data: AuthWithLoginAccess,
    Path(post_id): Path<String>,
    Query(query): Query<GetRepliesQuery>,
) -> CtxResult<Json<Vec<ReplyView>>> {
    let user = LocalUserDbService {
        db: &state.db.client,
        ctx: &auth_data.ctx,
    }
    .get_by_id(&auth_data.user_thing_id())
    .await?;

    let post_db_service = PostDbService {
        db: &state.db.client,
        ctx: &auth_data.ctx,
    };
    let post = post_db_service
        .get_view_by_id::<PostAccessView>(&post_id)
        .await?;

    if !PostAccess::new(&post).can_view(&user) {
        return Err(AppError::Forbidden.into());
    }

    let replies = state
        .db
        .replies
        .get(
            &auth_data.user_thing_id(),
            post.id.id.to_raw().as_ref(),
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

#[derive(Deserialize, Serialize, Validate)]
pub struct PostReplyInput {
    #[validate(length(min = 5, message = "Min 5 characters"))]
    pub content: String,
}

async fn create_reply(
    State(state): State<Arc<CtxState>>,
    auth_data: AuthWithLoginAccess,
    Path(post_id): Path<String>,
    JsonOrFormValidated(reply_input): JsonOrFormValidated<PostReplyInput>,
) -> CtxResult<Json<ReplyView>> {
    let user = LocalUserDbService {
        db: &state.db.client,
        ctx: &auth_data.ctx,
    }
    .get_by_id(&auth_data.user_thing_id())
    .await?;
    let created_by = user.id.as_ref().unwrap().clone();
    let post_db_service = PostDbService {
        db: &state.db.client,
        ctx: &auth_data.ctx,
    };
    let post = post_db_service
        .get_view_by_id::<PostAccessView>(&post_id)
        .await?;

    if !PostAccess::new(&post).can_create_reply(&user) {
        return Err(AppError::Forbidden.into());
    }

    let reply = state
        .db
        .replies
        .create(
            post.id.id.to_raw().as_ref(),
            user.id.as_ref().unwrap().id.to_raw().as_ref(),
            &reply_input.content,
        )
        .await?;

    let n_service = NotificationService::new(
        &state.db.client,
        &auth_data.ctx,
        &state.event_sender,
        &state.db.user_notifications,
    );

    let reply_view = ReplyView {
        id: reply.id,
        user: UserView::from(user),
        likes_nr: reply.likes_nr,
        content: reply.content,
        created_at: reply.created_at,
        updated_at: reply.updated_at,
        liked_by: None,
    };

    n_service
        .on_discussion_post_reply(&created_by, &post.id, &post.discussion.id, &reply_view)
        .await?;

    let post = post_db_service.increase_replies_nr(post.id.clone()).await?;

    n_service
        .on_discussion_post_reply_nr_increased(
            &created_by,
            &post.id.as_ref().unwrap(),
            &post.belongs_to,
            &post.replies_nr.to_string(),
        )
        .await?;

    Ok(Json(reply_view))
}

#[derive(Debug, Deserialize, Serialize)]
struct PostMember {
    user_ids: Vec<String>,
}

async fn add_members(
    auth_data: AuthWithLoginAccess,
    Path(post_id): Path<String>,
    State(ctx_state): State<Arc<CtxState>>,
    Json(body): Json<PostMember>,
) -> CtxResult<()> {
    let _ = PostService::new(
        &ctx_state.db.client,
        &auth_data.ctx,
        &ctx_state.event_sender,
        &ctx_state.db.user_notifications,
        ctx_state.file_storage.clone(),
        &ctx_state.db.tags,
        &ctx_state.db.likes,
        &ctx_state.db.access,
    )
    .add_members(&auth_data.user_thing_id(), &post_id, body.user_ids)
    .await?;

    Ok(())
}

async fn remove_members(
    auth_data: AuthWithLoginAccess,
    Path(post_id): Path<String>,
    State(ctx_state): State<Arc<CtxState>>,
    Json(body): Json<PostMember>,
) -> CtxResult<()> {
    let _ = PostService::new(
        &ctx_state.db.client,
        &auth_data.ctx,
        &ctx_state.event_sender,
        &ctx_state.db.user_notifications,
        ctx_state.file_storage.clone(),
        &ctx_state.db.tags,
        &ctx_state.db.likes,
        &ctx_state.db.access,
    )
    .remove_members(&auth_data.user_thing_id(), &post_id, body.user_ids)
    .await?;

    Ok(())
}

async fn get_members(
    auth_data: AuthWithLoginAccess,
    Path(post_id): Path<String>,
    State(ctx_state): State<Arc<CtxState>>,
) -> CtxResult<Json<Vec<UserView>>> {
    let users = PostService::new(
        &ctx_state.db.client,
        &auth_data.ctx,
        &ctx_state.event_sender,
        &ctx_state.db.user_notifications,
        ctx_state.file_storage.clone(),
        &ctx_state.db.tags,
        &ctx_state.db.likes,
        &ctx_state.db.access,
    )
    .get_users(&post_id, &auth_data.user_thing_id())
    .await?;

    Ok(Json(users))
}

async fn post_mark_as_deliver(
    Path(post_id): Path<String>,
    State(state): State<Arc<CtxState>>,
    auth_data: AuthWithLoginAccess,
) -> CtxResult<()> {
    let service = PostUserService::new(&state, &auth_data.ctx, &state.db.post_users);
    service
        .deliver(&auth_data.user_thing_id(), &post_id)
        .await?;
    Ok(())
}

async fn post_mark_as_read(
    Path(post_id): Path<String>,
    State(state): State<Arc<CtxState>>,
    auth_data: AuthWithLoginAccess,
) -> CtxResult<()> {
    let service = PostUserService::new(&state, &auth_data.ctx, &state.db.post_users);
    service.read(&auth_data.user_thing_id(), &post_id).await?;
    Ok(())
}
