use std::sync::Arc;

use crate::access::discussion::DiscussionAccess;
use crate::database::surrdb_utils::record_id_key_to_string;
use crate::entities::community::discussion_entity::{self, DiscussionType};
use crate::entities::community::post_entity::PostType;
use crate::entities::task_request::TaskRequestEntity;
use crate::entities::user_auth::local_user_entity::LocalUserDbService;
use crate::interfaces::repositories::task_request_ifce::TaskRequestRepositoryInterface;
use crate::middleware;
use crate::middleware::auth_with_login_access::AuthWithLoginAccess;
use crate::middleware::utils::db_utils::{Pagination, QryOrder};
use crate::models::view::access::DiscussionAccessView;
use crate::models::view::discussion::DiscussionView;
use crate::models::view::post::PostView;
use crate::models::view::task::TaskRequestView;
use crate::services::discussion_service::{CreateDiscussion, DiscussionService, UpdateDiscussion};
use crate::services::notification_service::NotificationService;
use crate::services::post_service::{GetPostsParams, PostInput, PostService};
use crate::services::task_service::{TaskRequestInput, TaskService};
use axum::extract::{DefaultBodyLimit, Path, Query, State};
use axum::routing::{delete, get, patch, post};
use axum::{Json, Router};
use axum_typed_multipart::TypedMultipart;
use discussion_entity::{Discussion, DiscussionDbService};
use middleware::ctx::Ctx;

use middleware::error::{AppError, CtxResult};
use middleware::mw_ctx::CtxState;
use middleware::utils::extractor_utils::JsonOrFormValidated;
use serde::{Deserialize, Serialize};
use validator::Validate;

pub fn routes(upload_max_size_mb: u64) -> Router<Arc<CtxState>> {
    let max_bytes_val = (1024 * 1024 * upload_max_size_mb) as usize;
    Router::new()
        .route("/api/discussions", get(get_discussions))
        .route("/api/discussions", post(create_discussion))
        .route("/api/discussions/{discussion_id}", patch(update_discussion))
        .route("/api/discussions/{discussion_id}", get(get_discussion))
        .route("/api/discussions/{discussion_id}/alias", post(update_alias))
        .route("/api/discussions/{discussion_id}/tasks", post(create_task))
        .route("/api/discussions/{discussion_id}/tasks", get(get_tasks))
        .route(
            "/api/discussions/{discussion_id}/chat_users",
            post(add_discussion_users),
        )
        .route(
            "/api/discussions/{discussion_id}/chat_users",
            delete(delete_discussion_users),
        )
        .route(
            "/api/discussions/{discussion_id}/posts",
            post(create_post).layer(DefaultBodyLimit::max(max_bytes_val)),
        )
        .route("/api/discussions/{discussion_id}/posts", get(get_posts))
        .route(
            "/api/discussions/{discussion_id}/posts/count",
            get(get_count_of_posts),
        )
}

async fn create_discussion(
    auth_data: AuthWithLoginAccess,
    State(state): State<Arc<CtxState>>,
    ctx: Ctx,
    Json(data): Json<CreateDiscussion>,
) -> CtxResult<Json<Discussion>> {
    let disc_service = DiscussionService::new(
        &state,
        &ctx,
        &state.db.access,
        &state.db.discussion_users,
        &state.db.user_notifications,
    );
    let disc = disc_service
        .create(&auth_data.user_thing_id(), data)
        .await?;
    Ok(Json(disc))
}

#[derive(Debug, Deserialize)]
pub struct GetDiscussionsQuery {
    r#type: Option<DiscussionType>,
    pub start: Option<u32>,
    pub count: Option<u16>,
    pub order_by: Option<String>,
    pub order_dir: Option<QryOrder>,
}

async fn get_discussions(
    auth_data: AuthWithLoginAccess,
    State(state): State<Arc<CtxState>>,
    Query(query): Query<GetDiscussionsQuery>,
) -> CtxResult<Json<Vec<DiscussionView>>> {
    let disc_service = DiscussionService::new(
        &state,
        &auth_data.ctx,
        &state.db.access,
        &state.db.discussion_users,
        &state.db.user_notifications,
    );
    let pagination = Pagination {
        order_by: query.order_by,
        order_dir: query.order_dir,
        count: query.count.unwrap_or(20),
        start: query.start.unwrap_or(0),
    };
    let discussions = disc_service
        .get(&auth_data.user_thing_id(), query.r#type, pagination)
        .await?;
    Ok(Json(discussions))
}

#[derive(Debug, Deserialize, Validate)]
struct DiscussionUsers {
    user_ids: Vec<String>,
}

async fn add_discussion_users(
    auth_data: AuthWithLoginAccess,
    Path(discussion_id): Path<String>,
    State(state): State<Arc<CtxState>>,
    JsonOrFormValidated(data): JsonOrFormValidated<DiscussionUsers>,
) -> CtxResult<()> {
    let disc_service = DiscussionService::new(
        &state,
        &auth_data.ctx,
        &state.db.access,
        &state.db.discussion_users,
        &state.db.user_notifications,
    );
    disc_service
        .add_chat_users(&auth_data.user_thing_id(), &discussion_id, data.user_ids)
        .await?;
    Ok(())
}

async fn delete_discussion_users(
    auth_data: AuthWithLoginAccess,
    Path(discussion_id): Path<String>,
    State(state): State<Arc<CtxState>>,
    JsonOrFormValidated(data): JsonOrFormValidated<DiscussionUsers>,
) -> CtxResult<()> {
    let disc_service = DiscussionService::new(
        &state,
        &auth_data.ctx,
        &state.db.access,
        &state.db.discussion_users,
        &state.db.user_notifications,
    );
    disc_service
        .remove_chat_users(&auth_data.user_thing_id(), &discussion_id, data.user_ids)
        .await?;
    Ok(())
}

async fn update_discussion(
    auth_data: AuthWithLoginAccess,
    State(state): State<Arc<CtxState>>,
    Path(discussion_id): Path<String>,
    Json(data): Json<UpdateDiscussion>,
) -> CtxResult<()> {
    let disc_service = DiscussionService::new(
        &state,
        &auth_data.ctx,
        &state.db.access,
        &state.db.discussion_users,
        &state.db.user_notifications,
    );
    disc_service
        .update(&auth_data.user_thing_id(), &discussion_id, data)
        .await?;

    Ok(())
}

#[derive(Debug, Deserialize, Serialize)]
pub struct UpdateAliasData {
    pub alias: Option<String>,
}
async fn update_alias(
    auth_data: AuthWithLoginAccess,
    State(state): State<Arc<CtxState>>,
    Path(discussion_id): Path<String>,
    Json(data): Json<UpdateAliasData>,
) -> CtxResult<()> {
    let disc_service = DiscussionService::new(
        &state,
        &auth_data.ctx,
        &state.db.access,
        &state.db.discussion_users,
        &state.db.user_notifications,
    );
    disc_service
        .update_alias(&auth_data.user_thing_id(), &discussion_id, data.alias)
        .await?;

    Ok(())
}

async fn get_tasks(
    auth_data: AuthWithLoginAccess,
    State(state): State<Arc<CtxState>>,
    Path(discussion_id): Path<String>,
) -> CtxResult<Json<Vec<TaskRequestView>>> {
    let user = LocalUserDbService {
        ctx: &auth_data.ctx,
        db: &state.db.client,
    }
    .get_by_id(&auth_data.user_thing_id())
    .await?;

    let disc_db_service = DiscussionDbService {
        ctx: &auth_data.ctx,
        db: &state.db.client,
    };

    let disc = disc_db_service
        .get_view_by_id::<DiscussionAccessView>(&discussion_id)
        .await?;

    if !DiscussionAccess::new(&disc).can_view(&user) {
        return Err(AppError::Forbidden.into());
    }

    let tasks = match disc.r#type {
        DiscussionType::Private => state
            .db
            .task_request
            .get_by_private_disc::<TaskRequestView>(
                &record_id_key_to_string(&disc.id.key),
                &record_id_key_to_string(&user.id.as_ref().unwrap().key),
                None,
                None,
                None,
                None,
            )
            .await
            .map_err(|e| AppError::SurrealDb {
                source: e.to_string(),
            })?,
        DiscussionType::Public => state
            .db
            .task_request
            .get_by_public_disc::<TaskRequestView>(
                &record_id_key_to_string(&disc.id.key),
                &record_id_key_to_string(&user.id.as_ref().unwrap().key),
                None,
                None,
                None,
                None,
            )
            .await
            .map_err(|e| AppError::SurrealDb {
                source: e.to_string(),
            })?,
    };

    Ok(Json(tasks))
}

async fn create_task(
    auth_data: AuthWithLoginAccess,
    State(state): State<Arc<CtxState>>,
    Path(discussion_id): Path<String>,
    Json(body): Json<TaskRequestInput>,
) -> CtxResult<Json<TaskRequestEntity>> {
    let task_service = TaskService::new(
        &state.db.client,
        &auth_data.ctx,
        &state.db.task_request,
        &state.db.task_donors,
        &state.db.task_participants,
        &state.db.access,
        &state.db.tags,
        NotificationService::new(
            &state.db.client,
            &auth_data.ctx,
            &state.event_sender,
            &state.db.user_notifications,
        ),
        state.file_storage.clone(),
    );

    let task = task_service
        .create_for_disc(&auth_data.user_thing_id(), &discussion_id, body)
        .await?;

    Ok(Json(task))
}

async fn create_post(
    auth_data: AuthWithLoginAccess,
    State(ctx_state): State<Arc<CtxState>>,
    Path(discussion_id): Path<String>,
    TypedMultipart(input_value): TypedMultipart<PostInput>,
) -> CtxResult<Json<PostView>> {
    let post_service = PostService::new(
        &ctx_state.db.client,
        &auth_data.ctx,
        &ctx_state.event_sender,
        &ctx_state.db.user_notifications,
        ctx_state.file_storage.clone(),
        &ctx_state.db.tags,
        &ctx_state.db.likes,
        &ctx_state.db.access,
        &ctx_state.db.discussion_users,
    );

    let post = post_service
        .create(&auth_data.user_thing_id(), &discussion_id, input_value)
        .await?;
    Ok(Json(post))
}

async fn get_posts(
    auth_data: AuthWithLoginAccess,
    State(ctx_state): State<Arc<CtxState>>,
    Path(disc_id): Path<String>,
    Query(query): Query<GetPostsParams>,
) -> CtxResult<Json<Vec<PostView>>> {
    let post_service = PostService::new(
        &ctx_state.db.client,
        &auth_data.ctx,
        &ctx_state.event_sender,
        &ctx_state.db.user_notifications,
        ctx_state.file_storage.clone(),
        &ctx_state.db.tags,
        &ctx_state.db.likes,
        &ctx_state.db.access,
        &ctx_state.db.discussion_users,
    );

    let posts = post_service
        .get_by_disc(&disc_id, &auth_data.user_thing_id(), query)
        .await?;

    Ok(Json(posts))
}

async fn get_discussion(
    auth_data: AuthWithLoginAccess,
    Path(discussion_id): Path<String>,
    State(state): State<Arc<CtxState>>,
) -> CtxResult<Json<DiscussionView>> {
    let disc_service = DiscussionService::new(
        &state,
        &auth_data.ctx,
        &state.db.access,
        &state.db.discussion_users,
        &state.db.user_notifications,
    );
    let data = disc_service
        .get_by_id(&discussion_id, &auth_data.user_thing_id())
        .await?;
    Ok(Json(data))
}

#[derive(Debug, Deserialize)]
pub struct GetCountPostsParams {
    pub user_id: String,
    pub filter_by_type: Option<PostType>,
}

async fn get_count_of_posts(
    auth_data: AuthWithLoginAccess,
    State(ctx_state): State<Arc<CtxState>>,
    Path(disc_id): Path<String>,
    Query(query): Query<GetCountPostsParams>,
) -> CtxResult<Json<u64>> {
    let post_service = PostService::new(
        &ctx_state.db.client,
        &auth_data.ctx,
        &ctx_state.event_sender,
        &ctx_state.db.user_notifications,
        ctx_state.file_storage.clone(),
        &ctx_state.db.tags,
        &ctx_state.db.likes,
        &ctx_state.db.access,
        &ctx_state.db.discussion_users,
    );

    let count = post_service
        .get_count(&disc_id, &query.user_id, query.filter_by_type)
        .await?;

    Ok(Json(count))
}
