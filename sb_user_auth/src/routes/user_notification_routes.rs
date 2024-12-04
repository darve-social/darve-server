use askama_axum::Template;
use axum::extract::State;
use axum::response::sse::Event;
use axum::response::Sse;
use axum::routing::get;
use axum::Router;
use futures::stream::Stream as FStream;
use serde::{Deserialize, Serialize};
use std::time::Duration;
use surrealdb::sql::Thing;
use surrealdb::Notification as SdbNotification;
use tokio_stream::StreamExt as _;

use crate::entity::local_user_entity::LocalUserDbService;
use crate::entity::user_notification_entitiy;
use crate::entity::user_notification_entitiy::{UserNotification, UserNotificationEvent};
use sb_middleware::ctx::Ctx;
use sb_middleware::error::CtxResult;
use sb_middleware::mw_ctx::CtxState;

pub fn routes(state: CtxState) -> Router {
    Router::new()
        .route("/api/notification/user/sse", get(user_notification_sse))
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
pub struct UserNotificationTaskCompleteView {
    task_id: Thing,
    delivered_by: Thing,
    requested_by: Thing,
    deliverables: Vec<String>,
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

async fn user_notification_sse(
    State(CtxState { _db, .. }): State<CtxState>,
    ctx: Ctx,
) -> CtxResult<Sse<impl FStream<Item=Result<Event, surrealdb::Error>>>> {
    let user = LocalUserDbService { db: &_db, ctx: &ctx }.get_ctx_user_thing().await?;

    let mut stream = _db.select(user_notification_entitiy::TABLE_NAME).live().await?
        .filter(move |r: &Result<SdbNotification<UserNotification>, surrealdb::Error>| {
            let notification = r.as_ref().unwrap().data.clone();
            if notification.user != user {
                return false;
            }

            true
        })
        .map(move |n: Result<SdbNotification<UserNotification>, surrealdb::Error>| {
            n.map(|n: surrealdb::Notification<UserNotification>| {
                let res = to_sse_event(ctx.clone(), n.data.event);
                if res.is_err() {
                    Event::default().data(res.unwrap_err().error.to_string()).event("Error".to_string())
                } else {
                    res.unwrap()
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

fn to_sse_event(ctx: Ctx, event: UserNotificationEvent) -> CtxResult<Event> {
    let event_ident = event.to_string();
    let event = match event {
        UserNotificationEvent::UserFollowAdded { username, follows_username } => {
            match ctx.to_htmx_or_json(UserNotificationFollowView { username, follows_username }) {
                Ok(response_string) => Event::default().data(response_string.0).event(event_ident),
                Err(err) => {
                    let msg = "ERROR rendering UserNotificationFollowView";
                    println!("{} ERR={}", &msg, err.error);
                    Event::default().data(msg).event("Error".to_string())
                }
            }
        }
        UserNotificationEvent::UserTaskRequestComplete { task_id, delivered_by, requested_by, deliverables } => {
            match ctx.to_htmx_or_json(UserNotificationTaskCompleteView {
                task_id,
                delivered_by,
                requested_by,
                deliverables,
            }) {
                Ok(res) => Event::default().data(res.0).event(event_ident),
                Err(err) => {
                    let msg = "ERROR rendering UserNotificationTaskCompleteView";
                    println!("{} ERR={}", &msg, err.error);
                    Event::default().data(msg).event("Error".to_string())
                }
            }
        }
        UserNotificationEvent::UserTaskRequestCreated { task_id, from_user, to_user } => {
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
        UserNotificationEvent::UserTaskRequestReceived { task_id, from_user, to_user } => {
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
    };
    Ok(event)
}
