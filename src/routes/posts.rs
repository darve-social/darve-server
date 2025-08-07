use std::sync::Arc;

use axum::extract::{DefaultBodyLimit, Path, Query, State};
use axum::routing::{delete, get, post};
use axum::{Json, Router};
use local_user_entity::LocalUserDbService;
use middleware::ctx::Ctx;
use middleware::error::CtxResult;
use middleware::mw_ctx::CtxState;
use post_entity::{Post, PostDbService};
use serde::{Deserialize, Serialize};
use validator::Validate;

use crate::entities::community::post_entity;
use crate::entities::task::task_request_entity::TaskRequest;
use crate::entities::user_auth::local_user_entity;
use crate::middleware;

use crate::middleware::auth_with_login_access::AuthWithLoginAccess;
use crate::middleware::utils::db_utils::{Pagination, QryOrder};
use crate::middleware::utils::extractor_utils::JsonOrFormValidated;
use crate::middleware::utils::string_utils::get_str_thing;
use crate::models::view::reply::ReplyView;
use crate::models::view::user::UserView;
use crate::routes::tasks::TaskRequestView;
use crate::services::notification_service::NotificationService;
use crate::services::post_service::{PostLikeData, PostService};
use crate::services::task_service::{TaskRequestInput, TaskService};

pub fn routes(upload_max_size_mb: u64) -> Router<Arc<CtxState>> {
    let max_bytes_val = (1024 * 1024 * upload_max_size_mb) as usize;
    Router::new()
        .route("/api/posts", get(get_posts))
        .route("/api/posts/:post_id/tasks", post(create_task))
        .route("/api/posts/:post_id/tasks", get(get_post_tasks))
        .route("/api/posts/:post_id/like", post(like))
        .route("/api/posts/:post_id/unlike", delete(unlike))
        .route("/api/posts/:post_id/replies", post(create_reply))
        .route("/api/posts/:post_id/replies", get(get_replies))
        .layer(DefaultBodyLimit::max(max_bytes_val))
}

#[derive(Debug, Deserialize)]
pub struct GetPostsQuery {
    pub tag: Option<String>,
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
    let post_thing = get_str_thing(&post_id)?;
    let task_service = TaskService::new(
        &state.db.client,
        &auth_data.ctx,
        &state.event_sender,
        &state.db.user_notifications,
        &state.db.task_donors,
        &state.db.task_participants,
        &state.db.task_relates,
    );

    let task = task_service
        .create(&auth_data.user_id, body, Some(post_thing.clone()))
        .await?;

    Ok(Json(task))
}

async fn get_post_tasks(
    auth_data: AuthWithLoginAccess,
    Path(post_id): Path<String>,
    State(state): State<Arc<CtxState>>,
) -> CtxResult<Json<Vec<TaskRequestView>>> {
    let post_db_service = PostDbService {
        ctx: &auth_data.ctx,
        db: &state.db.client,
    };
    let post = post_db_service.get_by_id_with_access(&post_id).await?;

    let tasks = state
        .db
        .task_relates
        .get_tasks_by_id::<TaskRequestView>(&post.id.as_ref().unwrap())
        .await?;
    Ok(Json(tasks))
}

async fn get_posts(
    Query(query): Query<GetPostsQuery>,
    State(state): State<Arc<CtxState>>,
    ctx: Ctx,
) -> CtxResult<Json<Vec<Post>>> {
    let post_db_service = PostDbService {
        ctx: &ctx,
        db: &state.db.client,
    };
    let pagination = Pagination {
        order_by: Some("id".to_string()),
        order_dir: query.order_dir,
        count: query.count.unwrap_or(100) as i8,
        start: query.start.unwrap_or_default() as i32,
    };
    let posts = post_db_service.get_by_tag(query.tag, pagination).await?;
    Ok(Json(posts))
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
        &ctx_state.file_storage,
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
        &ctx_state.file_storage,
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
    let _ = LocalUserDbService {
        db: &state.db.client,
        ctx: &auth_data.ctx,
    }
    .get_by_id(&auth_data.user_thing_id())
    .await?;

    let post = PostDbService {
        db: &state.db.client,
        ctx: &auth_data.ctx,
    }
    .get_by_id(&post_id)
    .await?;

    let replies = state
        .db
        .replies
        .get(
            post.id.as_ref().unwrap().id.to_raw().as_ref(),
            Pagination {
                order_by: None,
                order_dir: None,
                count: query.count.unwrap_or(50) as i8,
                start: query.start.unwrap_or(0) as i32,
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
    let created_by = user.id.as_ref().unwrap();
    let post_db_service = PostDbService {
        db: &state.db.client,
        ctx: &auth_data.ctx,
    };
    let post = post_db_service.get_by_id(&post_id).await?;

    let reply = state
        .db
        .replies
        .create(
            post.id.as_ref().unwrap().id.to_raw().as_ref(),
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
    n_service
        .on_discussion_post_reply(
            &created_by,
            &post.id.as_ref().unwrap(),
            &post.belongs_to.clone(),
            &reply_input.content,
            &post.discussion_topic.clone(),
        )
        .await?;

    n_service
        .on_discussion_post_reply_nr_increased(
            &created_by,
            &post.id.as_ref().unwrap(),
            &post.belongs_to.clone(),
            &post.replies_nr.to_string(),
            &post.discussion_topic.clone(),
        )
        .await?;

    Ok(Json(ReplyView {
        id: reply.id,
        user: UserView::from(user),
        content: reply.content,
        created_at: reply.created_at,
        updated_at: reply.updated_at,
    }))
}
