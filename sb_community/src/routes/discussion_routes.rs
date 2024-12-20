use std::collections::HashMap;
use std::str::FromStr;
use std::time::Duration;

use askama_axum::axum_core::response::IntoResponse;
use askama_axum::Template;
use axum::extract::{Path, Query, State};
use axum::response::sse::Event;
use axum::response::{Response, Sse};
use axum::routing::{get, post};
use axum::Router;
use axum_htmx::HX_REDIRECT;
use futures::stream::Stream as FStream;
use serde::{Deserialize, Serialize};
use surrealdb::sql::Thing;
use surrealdb::Notification as SdbNotification;
use tokio_stream::StreamExt as _;
use validator::Validate;

use crate::entity::community_entitiy::CommunityDbService;
use crate::entity::discussion_entitiy::{Discussion, DiscussionDbService};
use crate::entity::discussion_notification_entitiy;
use crate::entity::discussion_notification_entitiy::DiscussionNotification;
use crate::entity::post_entitiy::{Post, PostDbService};
use crate::routes::community_routes::DiscussionNotificationEvent;
use crate::routes::discussion_topic_routes::{
    DiscussionTopicItemForm, DiscussionTopicItemsEdit, DiscussionTopicView,
};
use sb_middleware::ctx::Ctx;
use sb_middleware::db::Db;
use sb_middleware::error::{AppError, CtxError, CtxResult};
use sb_middleware::mw_ctx::CtxState;
use sb_middleware::utils::db_utils::{IdentIdName, ViewFieldSelector, NO_SUCH_THING};
use sb_middleware::utils::extractor_utils::{DiscussionParams, JsonOrFormValidated};
use sb_middleware::utils::request_utils::CreatedResponse;
use sb_middleware::utils::string_utils::{get_string_thing, LEN_OR_NONE};
use sb_user_auth::entity::access_right_entity::AccessRightDbService;
use sb_user_auth::entity::access_rule_entity::{AccessRule, AccessRuleDbService};
use sb_user_auth::entity::authorization_entity::{
    is_any_ge_in_list, Authorization, AUTH_ACTIVITY_OWNER,
};
use sb_user_auth::entity::local_user_entity::LocalUserDbService;
use sb_user_auth::utils::askama_filter_util::filters;
use sb_user_auth::utils::template_utils::ProfileFormPage;

pub fn routes(state: CtxState) -> Router {
    let view_routes = Router::new().route(
        "/community/:community_id/discussion",
        get(create_update_form),
    )
    .route("/discussion/:discussion_id", get(display_discussion));

    Router::new()
        .merge(view_routes)
        .route("/api/discussion", post(create_update))
        .route("/api/discussion/:discussion_id/sse", get(discussion_sse))
        .route(
            "/api/discussion/:discussion_id/sse/htmx",
            get(discussion_sse_htmx),
        )
        .with_state(state)
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

#[derive(Deserialize, Serialize, Validate)]
pub struct DiscussionInput {
    pub id: String,
    pub community_id: String,
    #[validate(length(min = 5, message = "Min 5 characters"))]
    pub title: String,
    pub image_uri: Option<String>,
    // #[serde(skip_serializing_if = "Option::is_none")]
    // pub topics: Option<String>,
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
        "id, title, image_uri, [] as posts, topics.*.{id, title}, belongs_to, chat_room_user_ids,  latest_post_id.{id, title, content, media_links, created_by.*} as latest_post".to_string()
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
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DiscussionLatestPostCreatedBy {
    pub id: Thing,
    pub username: String,
}

// !! username is not set/valid
impl From<&Post> for DiscussionLatestPostView {
    fn from(value: &Post) -> Self {
        DiscussionLatestPostView {
            id: value.belongs_to.clone(),
            created_by: DiscussionLatestPostCreatedBy {
                id: value.created_by.clone(),
                username: value.created_by.clone().to_raw(),
            },
            title: value.title.clone(),
            content: value.content.clone(),
            media_links: value.media_links.clone(),
        }
    }
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

#[derive(Debug)]
struct DiscussionFormParams {
    id: Option<String>,
}

async fn display_discussion(
    State(CtxState { _db, .. }): State<CtxState>,
    ctx: Ctx,
    Path(discussion_id): Path<String>,
    q_params: DiscussionParams,
) -> CtxResult<axum::Json<DiscussionView>> {
    println!("->> {:<12} - get discussion", "HANDLER");

    let dis_view =
        get_discussion_view(&_db, &ctx, get_string_thing(discussion_id)?, q_params).await?;
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
    let (is_user_chat_discussion, user_auth) = is_user_chat_discussion__user_auths(
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
        .for_each(|mut discussion_post_view: &mut DiscussionPostView| {
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

async fn is_user_chat_discussion__user_auths(
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
    State(CtxState { _db, .. }): State<CtxState>,
    ctx: Ctx,
    Path(community_id): Path<String>,
    Query(mut qry): Query<HashMap<String, String>>,
) -> CtxResult<ProfileFormPage> {
    let user_id = LocalUserDbService {
        db: &_db,
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
            let required_disc_auth = Authorization {
                authorize_record_id: id.clone(),
                authorize_activity: AUTH_ACTIVITY_OWNER.to_string(),
                authorize_height: 1,
            };
            AccessRightDbService {
                db: &_db,
                ctx: &ctx,
            }
            .is_authorized(&user_id, &required_disc_auth)
            .await?;

            topics = DiscussionDbService {
                db: &_db,
                ctx: &ctx,
            }
            .get_topics(id.clone())
            .await?;
            get_discussion_view(
                &_db,
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
            let required_comm_auth = Authorization {
                authorize_record_id: comm_id.clone(),
                authorize_activity: AUTH_ACTIVITY_OWNER.to_string(),
                authorize_height: 1,
            };
            AccessRightDbService {
                db: &_db,
                ctx: &ctx,
            }
            .is_authorized(&user_id, &required_comm_auth)
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
        db: &_db,
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
    ))
}

async fn discussion_sse_htmx(
    State(ctx_state): State<CtxState>,
    ctx: Ctx,
    Path(discussion_id): Path<String>,
    q_params: DiscussionParams,
) -> CtxResult<Sse<impl FStream<Item = Result<Event, surrealdb::Error>>>> {
    let mut ctx = ctx.clone();
    ctx.is_htmx = true;
    discussion_sse(State(ctx_state), ctx, Path(discussion_id), q_params).await
}

async fn discussion_sse(
    State(CtxState { _db, .. }): State<CtxState>,
    ctx: Ctx,
    Path(discussion_id): Path<String>,
    q_params: DiscussionParams,
) -> CtxResult<Sse<impl FStream<Item = Result<Event, surrealdb::Error>>>> {
    let discussion_id = get_string_thing(discussion_id)?;
    let discussion = DiscussionDbService {
        db: &_db,
        ctx: &ctx,
    }
    .get(IdentIdName::Id(discussion_id))
    .await?;
    let discussion_id = discussion.id.expect("disc id");

    let (is_user_chat_discussion, user_auth) = is_user_chat_discussion__user_auths(
        &_db,
        &ctx,
        &discussion_id,
        discussion.chat_room_user_ids,
    )
    .await?;

    let mut stream = _db.select(discussion_notification_entitiy::TABLE_NAME).live().await?
        .filter(move |r: &Result<SdbNotification<DiscussionNotification>, surrealdb::Error>| {
            // TODO check if user still logged in
            // filter out events from other discussion - TODO make last events sub table for each discussion, delete all events older than ~5days
            let (event_discussion_id, event_topic_id) = match r.as_ref().unwrap().data.clone().event {
                DiscussionNotificationEvent::DiscussionPostAdded { discussion_id, topic_id, .. } => (discussion_id, topic_id),
                DiscussionNotificationEvent::DiscussionPostReplyAdded { discussion_id, topic_id, .. } => (discussion_id, topic_id),
                DiscussionNotificationEvent::DiscussionPostReplyNrIncreased { discussion_id, topic_id, .. } => (discussion_id, topic_id)
            };
            if event_discussion_id.ne(&discussion_id) {
                return false;
            }

            if q_params.topic_id.is_some() && q_params.topic_id.ne(&event_topic_id) {
                return false;
            }
            event_discussion_id.eq(&discussion_id)
        })
        .map(move |n: Result<SdbNotification<DiscussionNotification>, surrealdb::Error>| {
            n.map(|n: surrealdb::Notification<DiscussionNotification>| {
                let n_event = n.data.event;
                match n_event {
                    DiscussionNotificationEvent::DiscussionPostAdded { .. } => {
                        match serde_json::from_str::<DiscussionPostView>(&n.data.content) {
                            Ok(mut dpv) => {
                                dpv.viewer_access_rights = user_auth.clone();
                                dpv.has_view_access = match &dpv.access_rule {
                                    None => true,
                                    Some(ar) => is_user_chat_discussion || is_any_ge_in_list(&ar.authorization_required, &dpv.viewer_access_rights).unwrap_or(false)
                                };

                                match ctx.to_htmx_or_json(dpv) {
                                    Ok(post_html) => Event::default().data(post_html.0).event(n_event.to_string()),
                                    Err(err) => {
                                        let msg = "ERROR rendering DiscussionPostView";
                                        println!("{} ERR={}", &msg, err.error);
                                        Event::default().data(msg).event(SseEventName::get_error())
                                    }
                                }
                            }
                            Err(err) => {
                                let msg = "ERROR converting NotificationEvent content to DiscussionPostView";
                                println!("{} ERR={err}", &msg);
                                Event::default().data(msg).event(SseEventName::get_error())
                            }
                        }
                    }
                    DiscussionNotificationEvent::DiscussionPostReplyNrIncreased { .. } => {
                        Event::default().data(n.data.content).event(n_event.get_sse_event_ident())
                    }
                    DiscussionNotificationEvent::DiscussionPostReplyAdded { .. } => {
                        Event::default().data(n.data.content).event(n_event.get_sse_event_ident())
                    }
                }
            })
        }
        );
    // println!("GOT LIVE QRY STREAM");
    Ok(Sse::new(stream).keep_alive(
        axum::response::sse::KeepAlive::new()
            .interval(Duration::from_secs(10))
            .text("keep-alive-text"),
    ))
}

async fn create_update(
    State(CtxState { _db, .. }): State<CtxState>,
    ctx: Ctx,
    JsonOrFormValidated(form_value): JsonOrFormValidated<DiscussionInput>,
) -> CtxResult<Response> {
    println!("->> {:<12} - create_update_disc", "HANDLER");
    let local_user_db_service = LocalUserDbService {
        db: &_db,
        ctx: &ctx,
    };
    let aright_db_service = AccessRightDbService {
        db: &_db,
        ctx: &ctx,
    };
    let user_id = local_user_db_service.get_ctx_user_thing().await?;

    let disc_db_ser = DiscussionDbService {
        db: &_db,
        ctx: &ctx,
    };
    let community_db_service = CommunityDbService {
        db: &_db,
        ctx: &ctx,
    };

    let mut update_discussion = match form_value.id.len() > 0 {
        false => {
            // check if provided community id exists and has permissions on provided community id
            let comm_id_str = form_value.community_id.clone();
            let comm_id = get_string_thing(comm_id_str.clone())?;
            community_db_service
                .must_exist(IdentIdName::Id(comm_id.clone()))
                .await?;
            let required_comm_auth = Authorization {
                authorize_record_id: comm_id.clone(),
                authorize_activity: AUTH_ACTIVITY_OWNER.to_string(),
                authorize_height: 1,
            };
            aright_db_service
                .is_authorized(&user_id, &required_comm_auth)
                .await?;
            Discussion {
                id: None,
                belongs_to: comm_id,
                title: None,
                image_uri: None,
                topics: None,
                chat_room_user_ids: None,
                latest_post_id: None,
                r_created: None,
                created_by: user_id,
            }
        }
        true => {
            // check permissions in discussion and get community id from existing discussion in db
            let disc_id = get_string_thing(form_value.id)?;
            let required_disc_auth = Authorization {
                authorize_record_id: disc_id.clone(),
                authorize_activity: AUTH_ACTIVITY_OWNER.to_string(),
                authorize_height: 1,
            };
            aright_db_service
                .is_authorized(&user_id, &required_disc_auth)
                .await?;
            disc_db_ser.get(IdentIdName::Id(disc_id)).await?
        }
    };

    /* let topics = match form_value.topics {
        None => None,
        Some(topics_str) => {
            let t_str = topics_str.trim();
            match t_str.len() {
                0 => None,
                _ => {
                    let titles = t_str.split(",").into_iter().map(|s| s.trim().to_string()).collect();
                    let res = DiscussionTopicDbService { db: &_db, ctx: &ctx }.create_titles(titles).await?;
                    Some(res.into_iter().map(|dt| dt.id.unwrap()).collect())
                }
            }
        }
    };*/

    if form_value.title.len() > 0 {
        update_discussion.title = Some(form_value.title);
    } else {
        return Err(ctx.to_ctx_error(AppError::Generic {
            description: "title must have value".to_string(),
        }));
    };

    update_discussion.image_uri = form_value.image_uri.and_then(LEN_OR_NONE);

    let disc = disc_db_ser.create_update(update_discussion).await?;
    let res = CreatedResponse {
        success: true,
        id: disc.id.unwrap().to_raw(),
        uri: None,
    };
    let mut res = ctx.to_htmx_or_json::<CreatedResponse>(res)?.into_response();
    let comm = community_db_service
        .get(IdentIdName::Id(disc.belongs_to))
        .await?;

    res.headers_mut().append(
        HX_REDIRECT,
        format!("/community/{}", comm.name_uri)
            .as_str()
            .parse()
            .unwrap(),
    );
    Ok(res)
}
