use askama_axum::Template;
use axum::extract::State;
use axum::response::sse::Event;
use axum::response::Sse;
use axum::routing::get;
use axum::Router;
use futures::stream::Stream as FStream;
use futures::Stream;
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use std::clone::Clone;
use std::string::ToString;
use std::time::Duration;
use surrealdb::sql::Thing;
use surrealdb::{Error, Notification as SdbNotification};
use tokio_stream::StreamExt as _;

use crate::entity::local_user_entity::LocalUserDbService;
use crate::entity::user_notification_entitiy;
use crate::entity::user_notification_entitiy::{UserNotification, UserNotificationEvent};
use sb_middleware::ctx::Ctx;
use sb_middleware::db::Db;
use sb_middleware::error::{CtxError, CtxResult};
use sb_middleware::mw_ctx::CtxState;
use sb_middleware::utils::db_utils::NO_SUCH_THING;

pub fn routes(state: CtxState) -> Router {
    Router::new()
        // .route("/api/notification/user/sse", get(user_notification_sse))
        .with_state(state)
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
