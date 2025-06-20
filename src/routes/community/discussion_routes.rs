use std::collections::HashMap;
use std::convert::Infallible;
use std::sync::Arc;

use crate::database::client::Db;
use crate::entities::community::discussion_entity;
use crate::entities::community::post_entity::PostDbService;
use crate::entities::user_auth::{
    access_right_entity, access_rule_entity, authorization_entity, local_user_entity,
};
use crate::middleware::mw_ctx::AppEventType;
use crate::services::discussion_service::{CreateDiscussion, DiscussionService, UpdateDiscussion};
use crate::{middleware, utils};
use access_right_entity::AccessRightDbService;
use access_rule_entity::{AccessRule, AccessRuleDbService};
use askama_axum::Template;
use authorization_entity::{is_any_ge_in_list, Authorization, AUTH_ACTIVITY_OWNER};
use axum::extract::{Path, Query, State};
use axum::response::sse::{Event, KeepAlive};
use axum::response::Sse;
use axum::routing::{delete, get, patch, post};
use axum::{Json, Router};
use community_routes::DiscussionNotificationEvent;
use discussion_entity::{Discussion, DiscussionDbService};
use discussion_topic_routes::{
    DiscussionTopicItemForm, DiscussionTopicItemsEdit, DiscussionTopicView,
};
use futures::Stream;
use local_user_entity::LocalUserDbService;
use middleware::ctx::Ctx;

use middleware::error::{AppError, CtxError, CtxResult};
use middleware::mw_ctx::CtxState;
use middleware::utils::db_utils::{IdentIdName, ViewFieldSelector, NO_SUCH_THING};
use middleware::utils::extractor_utils::{DiscussionParams, JsonOrFormValidated};
use middleware::utils::string_utils::get_string_thing;
use serde::{Deserialize, Serialize};
use surrealdb::sql::Thing;
use tokio_stream::{wrappers::BroadcastStream, StreamExt};
use utils::askama_filter_util::filters;
use utils::template_utils::ProfileFormPage;
use validator::Validate;

use super::{community_routes, discussion_topic_routes};

pub fn routes() -> Router<Arc<CtxState>> {
    let view_routes = Router::new()
        .route(
            "/community/:community_id/discussion",
            get(create_update_form),
        )
        .route("/discussion/:discussion_id", get(display_discussion));

    Router::new()
        .merge(view_routes)
        .route("/api/discussions", get(get_discussions))
        .route("/api/discussions", post(create_discussion))
        .route("/api/discussions/:discussion_id", patch(update_discussion))
        .route("/api/discussions/:discussion_id", delete(delete_discussion))
        .route(
            "/api/discussions/:discussion_id/chat_users",
            post(add_discussion_users),
        )
        .route(
            "/api/discussions/:discussion_id/chat_users",
            delete(delete_discussion_users),
        )
        .route("/api/discussion/:discussion_id/sse", get(discussion_sse))
        .route(
            "/api/discussion/:discussion_id/sse/htmx",
            get(discussion_sse_htmx),
        )
}

// used in templates
pub struct SseEventName {}
impl SseEventName {
    pub fn get_discussion_post_added_event_name() -> String {
        DiscussionNotificationEvent::DiscussionPostAdded {
            discussion_id: NO_SUCH_THING.clone(),
            topic_id: None,
            post_id: NO_SUCH_THING.clone(),
        }
        .to_string()
    }
    pub fn get_discussion_post_reply_added(reply_ident: &Thing) -> String {
        DiscussionNotificationEvent::DiscussionPostReplyAdded {
            discussion_id: NO_SUCH_THING.clone(),
            topic_id: None,
            post_id: reply_ident.clone(),
        }
        .get_sse_event_ident()
    }
    pub fn get_discussion_post_reply_nr_increased(post_ident: &Thing) -> String {
        DiscussionNotificationEvent::DiscussionPostReplyNrIncreased {
            discussion_id: NO_SUCH_THING.clone(),
            topic_id: None,
            post_id: post_ident.clone(),
        }
        .get_sse_event_ident()
    }
    pub fn get_error() -> String {
        "Error".to_string()
    }
}

#[derive(Template, Serialize)]
#[template(path = "nera2/discussion_form.html")]
struct DiscussionForm {
    discussion_view: DiscussionView,
    topic_form: DiscussionTopicItemsEdit,
}

#[derive(Template, Serialize, Deserialize, Debug)]
#[template(path = "nera2/discussion_page.html")]
pub struct DiscussionPage {
    theme_name: String,
    window_title: String,
    nav_top_title: String,
    header_title: String,
    footer_text: String,
    discussion_view: DiscussionView,
}

#[derive(Template, Serialize, Deserialize, Debug)]
#[template(path = "nera2/discussion_view_1.html")]
pub struct DiscussionView {
    pub id: Option<Thing>,
    pub title: Option<String>,
    pub image_uri: Option<String>,
    pub belongs_to: Thing,
    pub chat_room_user_ids: Option<Vec<Thing>>,
    pub posts: Vec<DiscussionPostView>,
    pub latest_post: Option<DiscussionLatestPostView>,
    pub topics: Option<Vec<DiscussionTopicView>>,
    pub display_topic: Option<DiscussionTopicView>,
}

impl ViewFieldSelector for DiscussionView {
    fn get_select_query_fields(_ident: &IdentIdName) -> String {
        "id, title, image_uri, [] as posts, topics.*.{id, title}, belongs_to, chat_room_user_ids,  latest_post_id.{id, title, content, media_links, r_created, created_by.*} as latest_post".to_string()
    }
}

#[derive(Template, Serialize, Deserialize, Debug)]
#[template(path = "nera2/post_1_latest_chat.html")]
pub struct DiscussionLatestPostView {
    pub id: Thing,
    pub created_by: DiscussionLatestPostCreatedBy,
    pub title: String,
    pub content: String,
    pub media_links: Option<Vec<String>>,
    pub r_created: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DiscussionLatestPostCreatedBy {
    pub id: Thing,
    pub username: String,
    pub full_name: Option<String>,
    pub image_uri: Option<String>,
}

#[derive(Template, Serialize, Deserialize, Debug)]
#[template(path = "nera2/post-1-popup.html")]
pub struct DiscussionPostView {
    pub id: Thing,
    pub created_by_name: String,
    // belongs_to = discussion
    pub belongs_to_uri: Option<String>,
    pub belongs_to_id: Thing,
    pub title: String,
    pub r_title_uri: Option<String>,
    pub content: String,
    pub media_links: Option<Vec<String>>,
    pub r_created: String,
    pub replies_nr: i64,
    pub topic: Option<DiscussionTopicView>,
    pub access_rule: Option<AccessRule>,
    pub viewer_access_rights: Vec<Authorization>,
    pub has_view_access: bool,
}

impl ViewFieldSelector for DiscussionPostView {
    // post fields selct qry for view
    fn get_select_query_fields(_ident: &IdentIdName) -> String {
        "id, created_by.username as created_by_name, title, r_title_uri, content, media_links, r_created, belongs_to.name_uri as belongs_to_uri, belongs_to.id as belongs_to_id, replies_nr, discussion_topic.{id, title} as topic, discussion_topic.access_rule.* as access_rule, [] as viewer_access_rights, false as has_view_access".to_string()
    }
}

// not used anywhere - so commenting for now - @anukulpandey
// #[derive(Debug)]
// struct DiscussionFormParams {
//     id: Option<String>,
// }

async fn display_discussion(
    State(state): State<Arc<CtxState>>,
    ctx: Ctx,
    Path(discussion_id): Path<String>,
    Query(q_params): Query<DiscussionParams>,
) -> CtxResult<axum::Json<DiscussionView>> {
    let dis_view = get_discussion_view(
        &state.db.client,
        &ctx,
        get_string_thing(discussion_id)?,
        q_params,
    )
    .await?;
    Ok(axum::Json(dis_view))
}

pub async fn get_discussion_view(
    _db: &Db,
    ctx: &Ctx,
    discussion_id: Thing,
    q_params: DiscussionParams,
) -> CtxResult<DiscussionView> {
    let discussion_db_service = DiscussionDbService {
        db: &_db,
        ctx: &ctx,
    };
    let mut dis_template = discussion_db_service
        .get_view::<DiscussionView>(IdentIdName::Id(discussion_id.clone()))
        .await?;
    let disc_id =
        dis_template
            .id
            .clone()
            .ok_or(ctx.to_ctx_error(AppError::EntityFailIdNotFound {
                ident: discussion_id.to_raw(),
            }))?;
    let (is_user_chat_discussion, user_auth) = is_user_chat_discussion_user_auths(
        _db,
        ctx,
        &disc_id,
        dis_template.chat_room_user_ids.clone(),
    )
    .await?;

    dis_template.display_topic = if let Some(t_id) = q_params.topic_id.clone() {
        dis_template
            .topics
            .clone()
            .unwrap_or(vec![])
            .into_iter()
            .find(|t| t.id.eq(&t_id))
    } else {
        None
    };

    // TODO optimize with one qry
    let mut discussion_posts = PostDbService {
        db: &_db,
        ctx: &ctx,
    }
    .get_by_discussion_desc_view::<DiscussionPostView>(disc_id.clone(), q_params.clone())
    .await?;
    /*let user_auth = if is_user_chat_discussion {
        vec![Authorization{
            authorize_record_id: disc_id.clone(),
            authorize_activity: AUTH_ACTIVITY_OWNER.to_string(),
            authorize_height: 99,
        }]
    }else {
        get_user_discussion_auths(&_db, &ctx).await?
    };*/
    discussion_posts
        .iter_mut()
        .for_each(|discussion_post_view: &mut DiscussionPostView| {
            discussion_post_view.viewer_access_rights = user_auth.clone();
            discussion_post_view.has_view_access = match &discussion_post_view.access_rule {
                None => true,
                Some(ar) => {
                    is_user_chat_discussion
                        || is_any_ge_in_list(
                            &ar.authorization_required,
                            &discussion_post_view.viewer_access_rights,
                        )
                        .unwrap_or(false)
                }
            };
        });

    // println!("DSIIIIISSSS={:?}", dis_template.latest_post);
    dis_template.posts = discussion_posts;
    Ok(dis_template)
}

async fn is_user_chat_discussion_user_auths(
    db: &Db,
    ctx: &Ctx,
    discussion_id: &Thing,
    discussion_chat_room_user_ids: Option<Vec<Thing>>,
) -> CtxResult<(bool, Vec<Authorization>)> {
    let is_chat_disc = is_user_chat_discussion(ctx, &discussion_chat_room_user_ids)?;

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
    discussion_chat_room_user_ids: &Option<Vec<Thing>>,
) -> CtxResult<bool> {
    match discussion_chat_room_user_ids {
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

async fn create_update_form(
    State(state): State<Arc<CtxState>>,
    ctx: Ctx,
    Path(community_id): Path<String>,
    Query(qry): Query<HashMap<String, String>>,
) -> CtxResult<ProfileFormPage> {
    let user_id = LocalUserDbService {
        db: &state.db.client,
        ctx: &ctx,
    }
    .get_ctx_user_thing()
    .await?;

    let comm_id = get_string_thing(community_id.clone())?;
    let mut topics = vec![];
    let disc_id: Option<&String> = match qry.get("id").unwrap_or(&String::new()).len() > 0 {
        true => Some(qry.get("id").unwrap()),
        false => None,
    };

    let discussion_view = match disc_id {
        Some(id) => {
            let id = get_string_thing(id.clone())?;
            // auth discussion
           
            AccessRightDbService {
                db: &state.db.client,
                ctx: &ctx,
            }
            .has_owner_access(&user_id, &id.to_raw())
            .await?;

            topics = DiscussionDbService {
                db: &state.db.client,
                ctx: &ctx,
            }
            .get_topics(id.clone())
            .await?;
            get_discussion_view(
                &state.db.client,
                &ctx,
                id,
                DiscussionParams {
                    topic_id: None,
                    start: None,
                    count: None,
                },
            )
            .await?
        }
        None => {
            // auth community
            AccessRightDbService {
                db: &state.db.client,
                ctx: &ctx,
            }
            .has_owner_access(&user_id, &comm_id.to_raw())
            .await?;
            DiscussionView {
                id: None,
                title: None,
                image_uri: None,
                belongs_to: comm_id.clone(),
                chat_room_user_ids: None,
                posts: vec![],
                latest_post: None,
                topics: None,
                display_topic: None,
            }
        }
    };

    let access_rules = AccessRuleDbService {
        db: &state.db.client,
        ctx: &ctx,
    }
    .get_list(comm_id.clone())
    .await?;

    Ok(ProfileFormPage::new(
        Box::new(DiscussionForm {
            discussion_view,
            topic_form: DiscussionTopicItemsEdit {
                community_id: comm_id,
                edit_topic: DiscussionTopicItemForm {
                    id: String::new(),
                    discussion_id: match disc_id {
                        None => String::new(),
                        Some(id) => id.clone(),
                    },
                    title: String::new(),
                    hidden: false,
                    access_rule: None,
                    access_rules,
                },
                topics,
            },
        }),
        None,
        None,
        None,
    ))
}

async fn discussion_sse_htmx(
    State(ctx_state): State<Arc<CtxState>>,
    ctx: Ctx,
    Path(discussion_id): Path<String>,
    Query(q_params): Query<DiscussionParams>,
) -> CtxResult<Sse<impl Stream<Item = Result<Event, Infallible>>>> {
    let mut ctx = ctx.clone();
    ctx.is_htmx = true;
    discussion_sse(State(ctx_state), ctx, Path(discussion_id), Query(q_params)).await
}

async fn discussion_sse(
    State(ctx_state): State<Arc<CtxState>>,
    ctx: Ctx,
    Path(discussion_id): Path<String>,
    Query(q_params): Query<DiscussionParams>,
) -> CtxResult<Sse<impl Stream<Item = Result<Event, Infallible>>>> {
    let discussion_id = get_string_thing(discussion_id)?;
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
        discussion.chat_room_user_ids,
    )
    .await?;

    let rx = ctx_state.event_sender.subscribe();
    let stream = BroadcastStream::new(rx)
        .filter(move|msg| {
            if msg.is_err() {
                return false;
            }

            let event = match msg.as_ref().unwrap().clone().event {
                AppEventType::DiscussionNotificationEvent(e) => e,
                _ => return false,
            };

            let (event_discussion_id, event_topic_id) = match event {
                DiscussionNotificationEvent::DiscussionPostAdded {
                    discussion_id,
                    topic_id,
                    ..
                } => (discussion_id, topic_id),
                DiscussionNotificationEvent::DiscussionPostReplyAdded {
                    discussion_id,
                    topic_id,
                    ..
                } => (discussion_id, topic_id),
                DiscussionNotificationEvent::DiscussionPostReplyNrIncreased {
                    discussion_id,
                    topic_id,
                    ..
                } => (discussion_id, topic_id),
            };

            if event_discussion_id.ne(&discussion_id) {
                return false;
            }

            if q_params.topic_id.is_some() && q_params.topic_id.ne(&event_topic_id) {
                return false;
            }
            event_discussion_id.eq(&discussion_id)
        })
        .map(move |msg| {
            let event_opt = match msg {
                Err(_) => None,
                Ok(msg) => match msg.event {
                    AppEventType::DiscussionNotificationEvent(n) => match n {
                        DiscussionNotificationEvent::DiscussionPostAdded { .. } => {
                            match serde_json::from_str::<DiscussionPostView>(&msg.content.unwrap()) {
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
                                            Event::default().data(post_html.0).event(n.to_string()),
                                        ),
                                        Err(err) => {
                                            let msg = "ERROR rendering DiscussionPostView";
                                            println!("{} ERR={}", &msg, err.error);
                                            Some(
                                                Event::default()
                                                    .data(msg)
                                                    .event(SseEventName::get_error()),
                                            )
                                        }
                                    }
                                }
                                Err(err) => {
                                    let msg =
                                    "ERROR converting NotificationEvent content to DiscussionPostView";
                                    println!("{} ERR={err}", &msg);
                                    Some(Event::default().data(msg).event(SseEventName::get_error()))
                                }
                            }
                        }
                        DiscussionNotificationEvent::DiscussionPostReplyNrIncreased { .. } => Some(
                            Event::default()
                                .data(msg.content.unwrap())
                                .event(n.get_sse_event_ident()),
                        ),
                        DiscussionNotificationEvent::DiscussionPostReplyAdded { .. } => Some(
                            Event::default()
                                .data(msg.content.unwrap())
                                .event(n.get_sse_event_ident()),
                        ),
                    },
                    _ => None,
                },
            };
            Ok(event_opt.unwrap_or_else(|| {
                Event::default()
                    .data("No event".to_string())
                    .event(SseEventName::get_error())
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
    let disc = disc_service.create( data).await?;
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
    disc_service
        .delete( &discussion_id)
        .await?;
    Ok(())
}

async fn update_discussion(
    State(state): State<Arc<CtxState>>,
    ctx: Ctx,
    Path(discussion_id): Path<String>,
    Json(data): Json<UpdateDiscussion>,
) -> CtxResult<()> {
    let disc_service = DiscussionService::new(&state, &ctx);
    disc_service
        .update(&discussion_id, data)
        .await?;

    Ok(())
}
