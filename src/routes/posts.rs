use std::sync::Arc;

use askama_axum::Template;
use axum::extract::{DefaultBodyLimit, Path, Query, State};
use axum::routing::{delete, get, post};
use axum::{Json, Router};
use local_user_entity::LocalUserDbService;
use middleware::ctx::Ctx;
use middleware::error::CtxResult;
use middleware::mw_ctx::CtxState;
use middleware::utils::db_utils::IdentIdName;
use post_entity::{Post, PostDbService};
use serde::{Deserialize, Serialize};
use surrealdb::sql::Thing;
use validator::Validate;

use crate::entities::community::post_entity;
use crate::entities::community::reply_entity::{Reply, ReplyDbService};
use crate::entities::task::task_request_entity::TaskRequest;
use crate::entities::user_auth::local_user_entity;
use crate::middleware;

use crate::middleware::auth_with_login_access::AuthWithLoginAccess;
use crate::middleware::utils::db_utils::{Pagination, QryOrder, ViewFieldSelector};
use crate::middleware::utils::extractor_utils::JsonOrFormValidated;
use crate::middleware::utils::string_utils::get_str_thing;
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
    pub tag: String,
    pub order_dir: Option<QryOrder>,
    pub start: Option<u32>,
    pub count: Option<u16>,
}
#[derive(Template, Serialize, Deserialize, Debug)]
#[template(path = "nera2/post-reply-1.html")]
pub struct PostReplyView {
    pub id: Thing,
    pub username: String,
    pub title: String,
    pub content: String,
    pub r_created: String,
}
impl ViewFieldSelector for PostReplyView {
    fn get_select_query_fields() -> String {
        "id, title, content, r_created, created_by.username as username".to_string()
    }
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
    let posts = post_db_service.get_by_tag(&query.tag, pagination).await?;

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
        &ctx_state.db.tags,
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
        &ctx_state.db.tags,
    )
    .unlike(&post_id, &auth_data.user_thing_id())
    .await?;

    Ok(Json(PostLikeResponse { likes_count: count }))
}

async fn get_replies(
    State(state): State<Arc<CtxState>>,
    auth_data: AuthWithLoginAccess,
    Path(post_id): Path<String>,
) -> CtxResult<Json<Vec<PostReplyView>>> {
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

    let replies = ReplyDbService {
        db: &state.db.client,
        ctx: &auth_data.ctx,
    }
    .get_by_post_desc_view::<PostReplyView>(post.id.as_ref().unwrap().clone(), 0, 120)
    .await?;

    Ok(Json(replies))
}

#[derive(Deserialize, Serialize, Validate)]
pub struct PostReplyInput {
    #[validate(length(min = 5, message = "Min 5 characters"))]
    pub title: String,
    #[validate(length(min = 5, message = "Min 5 characters"))]
    pub content: String,
}

async fn create_reply(
    State(state): State<Arc<CtxState>>,
    auth_data: AuthWithLoginAccess,
    Path(post_id): Path<String>,
    JsonOrFormValidated(reply_input): JsonOrFormValidated<PostReplyInput>,
) -> CtxResult<Json<Reply>> {
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

    let reply_db_service = ReplyDbService {
        db: &state.db.client,
        ctx: &auth_data.ctx,
    };

    let reply = reply_db_service
        .create(Reply {
            id: None,
            discussion: post.belongs_to,
            belongs_to: post.id.as_ref().unwrap().clone(),
            created_by: created_by.clone(),
            title: reply_input.title,
            content: reply_input.content,
            r_created: None,
            r_updated: None,
        })
        .await?;
    let reply_comm_view = reply_db_service
        .get_view::<PostReplyView>(&IdentIdName::Id(reply.id.clone().unwrap()))
        .await?;

    let post = post_db_service
        .increase_replies_nr(post.id.as_ref().unwrap().clone())
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
            &reply.discussion.clone(),
            &reply_comm_view.render().unwrap(),
            &None,
        )
        .await?;

    n_service
        .on_discussion_post_reply_nr_increased(
            &created_by,
            &post.id.as_ref().unwrap(),
            &reply.discussion.clone(),
            &post.replies_nr.to_string(),
            &None,
        )
        .await?;

    Ok(Json(reply))
}
