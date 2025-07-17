use std::convert::Infallible;
use std::sync::Arc;

use crate::database::client::Db;
use crate::entities::community::discussion_entity;
use crate::entities::community::post_entity::Post;
use crate::entities::task::task_request_entity::TaskRequest;
use crate::entities::user_auth::{access_right_entity, authorization_entity, local_user_entity};
use crate::middleware;
use crate::middleware::mw_ctx::AppEventType;
use crate::routes::community::discussion_routes::DiscussionPostView;
use crate::routes::tasks::TaskRequestView;
use crate::services::discussion_service::{CreateDiscussion, DiscussionService, UpdateDiscussion};
use crate::services::post_service::{PostInput, PostService};
use crate::services::task_service::{TaskRequestInput, TaskService};
use access_right_entity::AccessRightDbService;
use authorization_entity::{is_any_ge_in_list, Authorization, AUTH_ACTIVITY_OWNER};
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

use middleware::error::{AppError, CtxError, CtxResult};
use middleware::mw_ctx::CtxState;
use middleware::utils::db_utils::IdentIdName;
use middleware::utils::extractor_utils::{DiscussionParams, JsonOrFormValidated};
use middleware::utils::string_utils::get_string_thing;
use serde::Deserialize;
use surrealdb::sql::Thing;
use tokio_stream::{wrappers::BroadcastStream, StreamExt};
use validator::Validate;

pub fn routes(upload_max_size_mb: u64) -> Router<Arc<CtxState>> {
    let max_bytes_val = (1024 * 1024 * upload_max_size_mb) as usize;
    Router::new()
        .route("/api/discussions", get(get_discussions))
        .route("/api/discussions", post(create_discussion))
        .route("/api/discussions/:discussion_id", patch(update_discussion))
        .route("/api/discussions/:discussion_id", delete(delete_discussion))
        .route("/api/discussions/:discussion_id/tasks", post(create_task))
        .route("/api/discussions/:discussion_id/tasks", get(get_tasks))
        .route(
            "/api/discussions/:discussion_id/chat_users",
            post(add_discussion_users),
        )
        .route(
            "/api/discussions/:discussion_id/chat_users",
            delete(delete_discussion_users),
        )
        .route("/api/discussions/:discussion_id/sse", get(discussion_sse))
        .route("/api/discussions/:discussion_id/posts", post(create_post))
        .layer(DefaultBodyLimit::max(max_bytes_val))
}

async fn is_user_chat_discussion_user_auths(
    db: &Db,
    ctx: &Ctx,
    discussion_id: &Thing,
    discussion_private_discussion_user_ids: Option<Vec<Thing>>,
) -> CtxResult<(bool, Vec<Authorization>)> {
    let is_chat_disc = is_user_chat_discussion(ctx, &discussion_private_discussion_user_ids)?;

    let user_auth = if is_chat_disc {
        vec![Authorization {
            authorize_record_id: discussion_id.clone(),
            authorize_activity: AUTH_ACTIVITY_OWNER.to_string(),
            authorize_height: 99,
        }]
    } else {
        get_user_discussion_auths(db, &ctx).await?
    };

    Ok((is_chat_disc, user_auth))
}

pub fn is_user_chat_discussion(
    ctx: &Ctx,
    discussion_private_discussion_user_ids: &Option<Vec<Thing>>,
) -> CtxResult<bool> {
    match discussion_private_discussion_user_ids {
        Some(chat_user_ids) => {
            let user_id = ctx.user_id()?;
            let is_in_chat_group =
                chat_user_ids.contains(&get_string_thing(user_id).expect("user id ok"));
            if !is_in_chat_group {
                return Err(ctx.to_ctx_error(AppError::AuthorizationFail {
                    required: "Is chat participant".to_string(),
                }));
            }
            Ok::<bool, CtxError>(true)
        }
        None => Ok(false),
    }
}

async fn get_user_discussion_auths(_db: &Db, ctx: &Ctx) -> CtxResult<Vec<Authorization>> {
    let user_auth = match ctx.user_id() {
        Ok(user_id) => {
            let user_id = get_string_thing(user_id)?;
            AccessRightDbService {
                db: &_db,
                ctx: &ctx,
            }
            .get_authorizations(&user_id)
            .await?
        }
        Err(_) => vec![],
    };
    Ok(user_auth)
}

pub async fn discussion_sse(
    State(ctx_state): State<Arc<CtxState>>,
    ctx: Ctx,
    Path(disc_id): Path<String>,
    Query(q_params): Query<DiscussionParams>,
) -> CtxResult<Sse<impl Stream<Item = Result<Event, Infallible>>>> {
    let discussion_id = get_string_thing(disc_id.clone())?;
    let discussion = DiscussionDbService {
        db: &ctx_state.db.client,
        ctx: &ctx,
    }
    .get(IdentIdName::Id(discussion_id))
    .await?;
    let discussion_id = discussion.id.expect("disc id");

    let (is_user_chat_discussion, user_auth) = is_user_chat_discussion_user_auths(
        &ctx_state.db.client,
        &ctx,
        &discussion_id,
        discussion.private_discussion_user_ids,
    )
    .await?;

    let rx = ctx_state.event_sender.subscribe();
    let stream = BroadcastStream::new(rx)
        .filter(move|msg| {
            if msg.is_err()  {
                return false;
            }

            let _ = match msg.as_ref().unwrap().clone().event {
                AppEventType::DiscussionPostAdded | AppEventType::DiscussionPostReplyAdded | AppEventType::DiscussionPostReplyNrIncreased => (),
                _ => return false,
            };

            let metadata = msg.as_ref().unwrap().metadata.as_ref().unwrap();

            if *metadata.discussion_id.as_ref().unwrap() != discussion_id {
                return false;
            }

            if q_params.topic_id.is_some() && q_params.topic_id.ne(&metadata.topic_id) {
                return false;
            }
            true
        })
        .map(move |msg| {
            let event_opt = match msg {
                Err(_) => None,
                Ok(msg) => match msg.event {
                        AppEventType::DiscussionPostAdded => {
                            match serde_json::from_str::<DiscussionPostView>(&msg.content.clone().unwrap()) {
                                Ok(mut dpv) => {
                                    dpv.viewer_access_rights = user_auth.clone();
                                    dpv.has_view_access = match &dpv.access_rule {
                                        None => true,
                                        Some(ar) => {
                                            is_user_chat_discussion
                                                || is_any_ge_in_list(
                                                    &ar.authorization_required,
                                                    &dpv.viewer_access_rights,
                                                )
                                                .unwrap_or(false)
                                        }
                                    };

                                    match ctx.to_htmx_or_json(dpv) {
                                        Ok(post_html) => Some(
                                            Event::default().event("DiscussionPostAdded").data(post_html.0),
                                        ),
                                        Err(err) => {
                                            let msg = "ERROR rendering DiscussionPostView";
                                            println!("{} ERR={}", &msg, err.error);
                                            Some(
                                                Event::default()
                                                    .data(&serde_json::to_string(&msg).unwrap())
                                            )
                                        }
                                    }
                                }
                                Err(err) => {
                                    let msg =
                                    "ERROR converting NotificationEvent content to DiscussionPostView";
                                    println!("{} ERR={err}", &msg);
                                    Some(Event::default().data(&serde_json::to_string(&msg).unwrap()))
                                }
                            }
                        }
                        AppEventType::DiscussionPostReplyNrIncreased => Some(
                            if ctx.is_htmx {
                                let metadata = msg.metadata.as_ref().unwrap();
                                let post_id = metadata.post_id.as_ref().unwrap().to_raw();
                                let id = format!("DiscussionPostReplyNrIncreased_{}",post_id);
                                    Event::default().event(id).data(&msg.content.unwrap())
                            } else {
                                Event::default()
                                .data(&serde_json::to_string(&msg).unwrap())
                            }
                        ),
                        AppEventType::DiscussionPostReplyAdded => Some(
                          if ctx.is_htmx {
                                Event::default().event("DiscussionPostReplyAdded") 
                                .data(&msg.content.unwrap())
                            } else {
                                Event::default()
                                .data(&serde_json::to_string(&msg).unwrap())
                            }
                        ),
                        _ => None,
                    },
            };
            Ok(event_opt.unwrap_or_else(|| {
                Event::default()
                    .data("No event".to_string())
            }))
        });

    Ok(Sse::new(stream).keep_alive(KeepAlive::default()))
}

async fn create_discussion(
    State(state): State<Arc<CtxState>>,
    ctx: Ctx,
    Json(data): Json<CreateDiscussion>,
) -> CtxResult<Json<Discussion>> {
    let disc_service = DiscussionService::new(&state, &ctx);
    let disc = disc_service.create(data).await?;
    Ok(Json(disc))
}

async fn get_discussions(
    State(state): State<Arc<CtxState>>,
    ctx: Ctx,
) -> CtxResult<Json<Vec<Discussion>>> {
    let local_user_db_service = LocalUserDbService {
        db: &state.db.client,
        ctx: &ctx,
    };
    let user_id = local_user_db_service.get_ctx_user_thing().await?;
    let disc_service = DiscussionService::new(&state, &ctx);
    let discussions = disc_service.get_by_chat_user(&user_id.to_raw()).await?;
    Ok(Json(discussions))
}

#[derive(Debug, Deserialize, Validate)]
struct DiscussionUsers {
    user_ids: Vec<String>,
}

async fn add_discussion_users(
    Path(discussion_id): Path<String>,
    State(state): State<Arc<CtxState>>,
    ctx: Ctx,
    JsonOrFormValidated(data): JsonOrFormValidated<DiscussionUsers>,
) -> CtxResult<()> {
    let disc_service = DiscussionService::new(&state, &ctx);
    disc_service
        .add_chat_users(&discussion_id, data.user_ids)
        .await?;
    Ok(())
}

async fn delete_discussion_users(
    Path(discussion_id): Path<String>,
    State(state): State<Arc<CtxState>>,
    ctx: Ctx,
    JsonOrFormValidated(data): JsonOrFormValidated<DiscussionUsers>,
) -> CtxResult<()> {
    let disc_service = DiscussionService::new(&state, &ctx);
    disc_service
        .remove_chat_users(&discussion_id, data.user_ids)
        .await?;
    Ok(())
}

async fn delete_discussion(
    State(state): State<Arc<CtxState>>,
    ctx: Ctx,
    Path(discussion_id): Path<String>,
) -> CtxResult<()> {
    let disc_service = DiscussionService::new(&state, &ctx);
    disc_service.delete(&discussion_id).await?;
    Ok(())
}

async fn update_discussion(
    State(state): State<Arc<CtxState>>,
    ctx: Ctx,
    Path(discussion_id): Path<String>,
    Json(data): Json<UpdateDiscussion>,
) -> CtxResult<()> {
    let disc_service = DiscussionService::new(&state, &ctx);
    disc_service.update(&discussion_id, data).await?;

    Ok(())
}

async fn get_tasks(
    ctx: Ctx,
    State(state): State<Arc<CtxState>>,
    Path(discussion_id): Path<String>,
) -> CtxResult<Json<Vec<TaskRequestView>>> {
    let user = LocalUserDbService {
        db: &state.db.client,
        ctx: &ctx,
    }
    .get_ctx_user()
    .await?;

    let disc_db_service = DiscussionDbService {
        db: &state.db.client,
        ctx: &ctx,
    };

    let disc = disc_db_service.get_by_id(&discussion_id).await?;

    let is_allow = match disc.private_discussion_user_ids {
        Some(user_ids) => user_ids.contains(user.id.as_ref().unwrap()),
        None => false,
    };

    if !is_allow {
        return Err(AppError::Generic {
            description: "Forbidden".to_string(),
        }
        .into());
    };
    let tasks = state
        .db
        .task_relates
        .get_tasks_by_id::<TaskRequestView>(&disc.id.as_ref().unwrap())
        .await?;

    Ok(Json(tasks))
}

async fn create_task(
    ctx: Ctx,
    State(state): State<Arc<CtxState>>,
    Path(discussion_id): Path<String>,
    Json(body): Json<TaskRequestInput>,
) -> CtxResult<Json<TaskRequest>> {
    let disc_thing = get_string_thing(discussion_id)?;

    let task_service = TaskService::new(
        &state.db.client,
        &ctx,
        &state.event_sender,
        &state.db.user_notifications,
        &state.db.task_donors,
        &state.db.task_participants,
        &state.db.task_relates,
    );

    let task = task_service
        .create(&ctx.user_id()?, body, Some(disc_thing.clone()))
        .await?;

    Ok(Json(task))
}

async fn create_post(
    ctx: Ctx,
    State(ctx_state): State<Arc<CtxState>>,
    Path(discussion_id): Path<String>,
    TypedMultipart(input_value): TypedMultipart<PostInput>,
) -> CtxResult<Json<Post>> {
    let user_id = ctx.user_thing_id()?;

    let post_service = PostService::new(
        &ctx_state.db.client,
        &ctx,
        &ctx_state.event_sender,
        &ctx_state.db.user_notifications,
        &ctx_state.file_storage,
    );

    let post = post_service
        .create(&user_id, &discussion_id, input_value)
        .await?;

    Ok(Json(post))
}
