use std::convert::Infallible;
use std::sync::Arc;

use crate::access::discussion::DiscussionAccess;
use crate::entities::community::discussion_entity::{self, DiscussionType};
use crate::entities::task::task_request_entity::{TaskRequest, TaskRequestDbService};
use crate::entities::user_auth::local_user_entity;
use crate::middleware;
use crate::middleware::auth_with_login_access::AuthWithLoginAccess;
use crate::middleware::mw_ctx::AppEventType;
use crate::middleware::utils::db_utils::{Pagination, QryOrder};
use crate::models::view::access::DiscussionAccessView;
use crate::models::view::discussion::DiscussionView;
use crate::models::view::post::PostView;
use crate::models::view::task::TaskRequestView;
use crate::services::discussion_service::{CreateDiscussion, DiscussionService, UpdateDiscussion};
use crate::services::post_service::{GetPostsParams, PostInput, PostService};
use crate::services::task_service::{TaskRequestInput, TaskService};
use axum::extract::{DefaultBodyLimit, Path, Query, State};
use axum::response::sse::{Event, KeepAlive};
use axum::response::Sse;
use axum::routing::{delete, get, patch, post};
use axum::{Json, Router};
use axum_typed_multipart::TypedMultipart;
use discussion_entity::{Discussion, DiscussionDbService};
use futures::Stream;
use local_user_entity::LocalUserDbService;
use middleware::ctx::Ctx;

use middleware::error::{AppError, CtxResult};
use middleware::mw_ctx::CtxState;
use middleware::utils::extractor_utils::JsonOrFormValidated;
use serde::{Deserialize, Serialize};
use tokio_stream::{wrappers::BroadcastStream, StreamExt};
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
        .route("/api/discussions/{discussion_id}/sse", get(discussion_sse))
        .route(
            "/api/discussions/{discussion_id}/posts",
            post(create_post).layer(DefaultBodyLimit::max(max_bytes_val)),
        )
        .route("/api/discussions/{discussion_id}/posts", get(get_posts))
}

pub async fn discussion_sse(
    auth_data: AuthWithLoginAccess,
    State(ctx_state): State<Arc<CtxState>>,
    ctx: Ctx,
    Path(disc_id): Path<String>,
) -> CtxResult<Sse<impl Stream<Item = Result<Event, Infallible>>>> {
    let user = LocalUserDbService {
        db: &ctx_state.db.client,
        ctx: &ctx,
    }
    .get_by_id(&auth_data.user_thing_id())
    .await?;

    let discussion = DiscussionDbService {
        db: &ctx_state.db.client,
        ctx: &ctx,
    }
    .get_view_by_id::<DiscussionAccessView>(&disc_id)
    .await?;

    if !DiscussionAccess::new(&discussion).can_view(&user) {
        return Err(AppError::Forbidden.into());
    }

    let discussion_id = discussion.id;

    let rx = ctx_state.event_sender.subscribe();
    let stream = BroadcastStream::new(rx)
        .filter(move |msg| {
            if msg.is_err() {
                return false;
            }

            let _ = match msg.as_ref().unwrap().clone().event {
                AppEventType::DiscussionPostAdded
                | AppEventType::DiscussionPostReplyAdded
                | AppEventType::DiscussionPostReplyNrIncreased => (),
                _ => return false,
            };

            let metadata = msg.as_ref().unwrap().metadata.as_ref().unwrap();

            if *metadata.discussion_id.as_ref().unwrap() != discussion_id {
                return false;
            }

            true
        })
        .map(move |msg| {
            let event_opt = match msg {
                Err(_) => None,
                Ok(msg) => match msg.event {
                    AppEventType::DiscussionPostAdded => {
                        match serde_json::from_str::<PostView>(&msg.content.clone().unwrap()) {
                            Ok(_) => Some(if ctx.is_htmx {
                                Event::default()
                                    .event("DiscussionPostAdded")
                                    .data(msg.content.unwrap())
                            } else {
                                Event::default().data(&serde_json::to_string(&msg).unwrap())
                            }),
                            Err(err) => {
                                let msg = "ERROR converting NotificationEvent content to PostView";
                                println!("{} ERR={err}", &msg);
                                Some(Event::default().data(&serde_json::to_string(&msg).unwrap()))
                            }
                        }
                    }
                    AppEventType::DiscussionPostReplyNrIncreased => Some(if ctx.is_htmx {
                        let metadata = msg.metadata.as_ref().unwrap();
                        let post_id = metadata.post_id.as_ref().unwrap().to_raw();
                        let id = format!("DiscussionPostReplyNrIncreased_{}", post_id);
                        Event::default().event(id).data(&msg.content.unwrap())
                    } else {
                        Event::default().data(&serde_json::to_string(&msg).unwrap())
                    }),
                    AppEventType::DiscussionPostReplyAdded => Some(if ctx.is_htmx {
                        Event::default()
                            .event("DiscussionPostReplyAdded")
                            .data(&msg.content.unwrap())
                    } else {
                        Event::default().data(&serde_json::to_string(&msg).unwrap())
                    }),
                    _ => None,
                },
            };
            Ok(event_opt.unwrap_or_else(|| Event::default().data("No event".to_string())))
        });

    Ok(Sse::new(stream).keep_alive(KeepAlive::default()))
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

    let task_db_service = TaskRequestDbService {
        ctx: &auth_data.ctx,
        db: &state.db.client,
    };

    let tasks: Vec<_> = task_db_service
        .get_by_disc(disc.id, user.id.as_ref().unwrap().clone())
        .await?;

    Ok(Json(tasks))
}

async fn create_task(
    auth_data: AuthWithLoginAccess,
    State(state): State<Arc<CtxState>>,
    Path(discussion_id): Path<String>,
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
        &state.db.tags,
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
