use askama_axum::Template;
use axum::extract::State;
use axum::response::sse::Event;
use axum::response::{Html, Sse};
use axum::routing::get;
use axum::Router;
use futures::stream::Stream as FStream;
use futures::Stream;
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use std::clone::Clone;
use std::string::ToString;
use std::time::Duration;
use surrealdb::sql::{Thing};
use surrealdb::{Error, Notification as SdbNotification};
use tokio_stream::StreamExt as _;

use crate::entity::local_user_entity::LocalUserDbService;
use crate::entity::user_notification_entitiy;
use crate::entity::user_notification_entitiy::{
    UserNotification, UserNotificationDbService, UserNotificationEvent,
};
use sb_middleware::ctx::Ctx;
use sb_middleware::db::Db;
use sb_middleware::error::{AppError, CtxError, CtxResult};
use sb_middleware::mw_ctx::CtxState;
use sb_middleware::utils::db_utils::{IdentIdName, ViewFieldSelector, NO_SUCH_THING};
use sb_middleware::utils::extractor_utils::DiscussionParams;

pub fn routes(state: CtxState) -> Router {
    Router::new()
        .route("/api/notification/user/sse", get(user_notification_sse))
        .route(
            "/api/notification/user/history",
            get(user_notification_history),
        )
        .with_state(state)
}

#[derive(Serialize, Deserialize, Debug)]
pub struct UserNotificationView {
    pub id: Thing,
    pub user: UserView,
    pub event: UserNotificationEvent,
    pub task: Option<NotifTaskRequestView>,
    pub delivered_by: Option<UserView>,
    pub deliverable: Option<NotifDeliverableView>,
    pub from_user: Option<UserView>,
    pub to_user: Option<UserView>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct UserView {
    pub id: Thing,
    pub username: String,
    pub full_name: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct NotifDeliverableView {
    pub id: Thing,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub urls: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub post: Option<NotifProfilePostView>,
    // pub user: UserView,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct NotifProfilePostView {
    pub id: Thing,
    // pub username: Option<String>,
    // belongs_to_id=discussion
    // pub belongs_to_id: Thing,
    pub r_title_uri: Option<String>,
    pub title: Option<String>,
    pub content: String,
    pub media_links: Option<Vec<String>>,
    // pub r_created: String,
}

#[derive(Deserialize, Serialize, Debug)]
pub struct NotifTaskRequestView {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<Thing>,
    // pub from_user: sb_task::routes::task_request_routes::UserView,
    // #[serde(skip_serializing_if = "Option::is_none")]
    // pub to_user: Option<sb_task::routes::task_request_routes::UserView>,
    // #[serde(skip_serializing_if = "Option::is_none")]
    pub request_post: Option<Thing>,
    pub request_txt: String,
    // pub participants: Vec<TaskRequestParticipationView>,
    // pub status: String,
    // pub reward_type: RewardType,
    // pub valid_until: DateTime<Utc>,
    // pub currency: CurrencySymbol,
    // #[serde(skip_serializing_if = "Option::is_none")]
    // pub deliverables: Option<Vec<DeliverableView>>,
    // #[serde(skip_serializing_if = "Option::is_none")]
    // pub r_created: Option<String>,
    // #[serde(skip_serializing_if = "Option::is_none")]
    // pub r_updated: Option<String>,
}

impl ViewFieldSelector for UserNotificationView {
    fn get_select_query_fields(_ident: &IdentIdName) -> String {
        "id, user.* as user, event, event.value.delivered_by.* as delivered_by,\
           event.d.value.task_id.{id, request_post, request_txt } as task,\
           event.value.deliverable.{id, urls, post.{id, r_title_uri, title, media_links, content}} as deliverable,\
           event.value.from_user.* as from_user, event.value.to_user.* as to_user ".to_string()
    }
}
#[derive(Template, Serialize, Deserialize, Debug)]
#[template(path = "nera2/user_notification_follow_view_1.html")]
pub struct UserNotificationFollowView {
    username: String,
    follows_username: String,
}

#[derive(Template, Serialize, Deserialize, Debug)]
#[template(path = "nera2/user_notification_task_request_complete_view_1.html")]
pub struct UserNotificationTaskDeliveredView {
    task_id: Thing,
    delivered_by: Thing,
    deliverable: Thing,
}

#[derive(Template, Serialize, Deserialize, Debug)]
#[template(path = "nera2/user_notification_task_request_created_view_1.html")]
pub struct UserNotificationTaskCreatedView {
    task_id: Thing,
    from_user: Thing,
    to_user: Thing,
}

#[derive(Template, Serialize, Deserialize, Debug)]
#[template(path = "nera2/user_notification_task_request_received_view_1.html")]
pub struct UserNotificationTaskReceivedView {
    task_id: Thing,
    from_user: Thing,
    to_user: Thing,
}

static ACCEPT_EVENT_NAMES: Lazy<[String; 4]> = Lazy::new(|| {
    [
        UserNotificationEvent::UserTaskRequestDelivered {
            task_id: NO_SUCH_THING.clone(),
            deliverable: NO_SUCH_THING.clone(),
            delivered_by: NO_SUCH_THING.clone(),
        }
        .to_string(),
        UserNotificationEvent::UserTaskRequestReceived {
            task_id: NO_SUCH_THING.clone(),
            from_user: NO_SUCH_THING.clone(),
            to_user: NO_SUCH_THING.clone(),
        }
        .to_string(),
        UserNotificationEvent::UserFollowAdded {
            username: "".to_string(),
            follows_username: "".to_string(),
        }
        .to_string(),
        UserNotificationEvent::UserTaskRequestCreated {
            task_id: NO_SUCH_THING.clone(),
            from_user: NO_SUCH_THING.clone(),
            to_user: NO_SUCH_THING.clone(),
        }
        .to_string(),
    ]
});

async fn user_notification_history(
    State(CtxState { _db, .. }): State<CtxState>,
    ctx: Ctx,
    q_params: DiscussionParams,
) -> CtxResult<Html<String>> {
    let user = LocalUserDbService {
        db: &_db,
        ctx: &ctx,
    }
    .get_ctx_user_thing()
    .await?;
    let notifications = UserNotificationDbService {
        db: &_db,
        ctx: &ctx,
    }
    .get_by_user_view::<UserNotificationView>(user, q_params)
    .await?;
    let json = serde_json::to_string(&notifications).map_err(|_| {
        ctx.to_ctx_error(AppError::Generic {
            description: "Render json error".to_string(),
        })
    })?;
    Ok(Html(json))
}

async fn user_notification_sse(
    State(CtxState { _db, .. }): State<CtxState>,
    ctx: Ctx,
) -> CtxResult<Sse<impl FStream<Item = Result<Event, surrealdb::Error>>>> {
    create_user_notifications_sse(
        &_db,
        ctx.clone(),
        Vec::from(ACCEPT_EVENT_NAMES.clone()),
        to_sse_event,
    )
    .await?
}

pub async fn create_user_notifications_sse(
    db: &Db,
    ctx: Ctx,
    accept_events: Vec<String>,
    to_sse_fn: fn(Ctx, UserNotification) -> CtxResult<Event>,
) -> Result<Result<Sse<impl Stream<Item = Result<Event, Error>> + Sized>, CtxError>, CtxError> {
    let user = LocalUserDbService { db, ctx: &ctx }
        .get_ctx_user_thing()
        .await?;

    let stream = db
        .select(user_notification_entitiy::TABLE_NAME)
        .live()
        .await?
        .filter(
            move |r: &Result<SdbNotification<UserNotification>, surrealdb::Error>| {
                let notification = r.as_ref().unwrap().data.clone();
                // filter out chat messages since they are in profile route
                notification.user == user && accept_events.contains(&notification.event.to_string())
            },
        )
        .map(
            move |n: Result<SdbNotification<UserNotification>, surrealdb::Error>| {
                n.map(|n: surrealdb::Notification<UserNotification>| {
                    let res = to_sse_fn(ctx.clone(), n.data);
                    if res.is_err() {
                        Event::default()
                            .data(res.unwrap_err().error.to_string())
                            .event("Error".to_string())
                    } else {
                        res.unwrap()
                    }
                })
            },
        );

    // println!("GOT LIVE QRY STREAM");
    Ok(Ok(Sse::new(stream).keep_alive(
        axum::response::sse::KeepAlive::new()
            .interval(Duration::from_secs(10))
            .text("keep-alive-text"),
    )))
}

fn to_sse_event(ctx: Ctx, notification: UserNotification) -> CtxResult<Event> {
    let event_ident = notification.event.to_string();
    let event = match notification.event {
        UserNotificationEvent::UserFollowAdded {
            username,
            follows_username,
        } => {
            match ctx.to_htmx_or_json(UserNotificationFollowView {
                username,
                follows_username,
            }) {
                Ok(response_string) => Event::default().data(response_string.0).event(event_ident),
                Err(err) => {
                    let msg = "ERROR rendering UserNotificationFollowView";
                    println!("{} ERR={}", &msg, err.error);
                    Event::default().data(msg).event("Error".to_string())
                }
            }
        }
        UserNotificationEvent::UserTaskRequestDelivered {
            task_id,
            deliverable,
            delivered_by,
        } => {
            match ctx.to_htmx_or_json(UserNotificationTaskDeliveredView {
                task_id,
                delivered_by,
                deliverable,
            }) {
                Ok(res) => Event::default().data(res.0).event(event_ident),
                Err(err) => {
                    let msg = "ERROR rendering UserNotificationTaskDeliveredView";
                    println!("{} ERR={}", &msg, err.error);
                    Event::default().data(msg).event("Error".to_string())
                }
            }
        }
        UserNotificationEvent::UserTaskRequestCreated {
            task_id,
            from_user,
            to_user,
        } => {
            match ctx.to_htmx_or_json(UserNotificationTaskCreatedView {
                task_id,
                from_user,
                to_user,
            }) {
                Ok(res) => Event::default().data(res.0).event(event_ident),
                Err(err) => {
                    let msg = "ERROR rendering UserNotificationTaskCreatedView";
                    println!("{} ERR={}", &msg, err.error);
                    Event::default().data(msg).event("Error".to_string())
                }
            }
        }
        UserNotificationEvent::UserTaskRequestReceived {
            task_id,
            from_user,
            to_user,
        } => {
            match ctx.to_htmx_or_json(UserNotificationTaskReceivedView {
                task_id,
                from_user,
                to_user,
            }) {
                Ok(res) => Event::default().data(res.0).event(event_ident),
                Err(err) => {
                    let msg = "ERROR rendering UserNotificationTaskReceivedView";
                    println!("{} ERR={}", &msg, err.error);
                    Event::default().data(msg).event("Error".to_string())
                }
            }
        }
        _ => Event::default()
            .data(format!("Event ident {event_ident} recognised"))
            .event("Error".to_string()),
    };
    Ok(event)
}
