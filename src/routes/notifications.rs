use crate::interfaces::repositories::discussion_user::DiscussionUserRepositoryInterface;
use crate::interfaces::repositories::user_notifications::{
    GetNotificationOptions, UserNotificationsInterface,
};
use crate::middleware;
use crate::middleware::auth_with_login_access::AuthWithLoginAccess;
use crate::middleware::mw_ctx::AppEventType;
use crate::middleware::mw_ctx::CtxState;
use crate::middleware::utils::db_utils::QryOrder;
use crate::models::view::notification::UserNotificationView;
use crate::utils::user_presence_guard::UserPresenceGuard;
use axum::extract::{Path, Query, State};
use axum::response::sse::{Event, KeepAlive};
use axum::response::Sse;
use axum::routing::{get, post};
use axum::{Json, Router};
use chrono::{DateTime, Utc};
use futures::{stream, Stream};
use middleware::ctx::Ctx;
use serde::Deserialize;
use serde_json::json;
use std::convert::Infallible;
use std::sync::Arc;
use tokio_stream::wrappers::BroadcastStream;

use crate::{
    entities::user_auth::local_user_entity::LocalUserDbService, middleware::error::CtxResult,
};
use futures::StreamExt;

pub fn routes() -> Router<Arc<CtxState>> {
    Router::new()
        .route("/api/notifications", get(get_notifications))
        .route("/api/notifications/read", post(read_all))
        .route("/api/notifications/sse", get(sse))
        .route("/api/notifications/count", get(get_count))
        .route("/api/notifications/:notification_id/read", post(read))
}

async fn read(
    Path(notification_id): Path<String>,
    State(state): State<Arc<CtxState>>,
    ctx: Ctx,
) -> CtxResult<()> {
    let user = LocalUserDbService {
        db: &state.db.client,
        ctx: &ctx,
    }
    .get_ctx_user_thing()
    .await?;

    let _ = state
        .db
        .user_notifications
        .read(&notification_id, &user.id.to_raw())
        .await?;

    Ok(())
}

async fn read_all(State(state): State<Arc<CtxState>>, ctx: Ctx) -> CtxResult<()> {
    let user = LocalUserDbService {
        db: &state.db.client,
        ctx: &ctx,
    }
    .get_ctx_user_thing()
    .await?;

    let _ = state.db.user_notifications.read_all(&user.to_raw()).await?;

    Ok(())
}

#[derive(Debug, Deserialize)]
struct GetNotificationsQuery {
    start: Option<DateTime<Utc>>,
    count: Option<u8>,
    is_read: Option<bool>,
    order_dir: Option<QryOrder>,
}

async fn get_notifications(
    State(state): State<Arc<CtxState>>,
    auth_data: AuthWithLoginAccess,
    Query(query): Query<GetNotificationsQuery>,
) -> CtxResult<Json<Vec<UserNotificationView>>> {
    let _ = LocalUserDbService {
        db: &state.db.client,
        ctx: &auth_data.ctx,
    }
    .exists_by_id(&auth_data.user_thing_id())
    .await?;

    let notifications = state
        .db
        .user_notifications
        .get_by_user(
            &auth_data.user_thing_id(),
            GetNotificationOptions {
                limit: query.count.unwrap_or(50),
                start: query.start.unwrap_or(Utc::now()),
                order_dir: query.order_dir.map_or(QryOrder::DESC, |v| v),
                is_read: query.is_read,
            },
        )
        .await?;

    Ok(Json(notifications))
}

#[derive(Debug, Deserialize)]
struct GetCountQuery {
    is_read: Option<bool>,
}

async fn get_count(
    State(state): State<Arc<CtxState>>,
    auth_data: AuthWithLoginAccess,
    Query(query): Query<GetCountQuery>,
) -> CtxResult<Json<u64>> {
    let _ = LocalUserDbService {
        db: &state.db.client,
        ctx: &auth_data.ctx,
    }
    .exists_by_id(&auth_data.user_thing_id())
    .await?;

    let count = state
        .db
        .user_notifications
        .get_count(&auth_data.user_id, query.is_read)
        .await?;

    Ok(Json(count))
}

async fn sse(
    State(state): State<Arc<CtxState>>,
    auth_data: AuthWithLoginAccess,
) -> CtxResult<Sse<impl Stream<Item = Result<Event, Infallible>>>> {
    let user = LocalUserDbService {
        db: &state.db.client,
        ctx: &auth_data.ctx,
    }
    .get_ctx_user_thing()
    .await?;

    let user_id = user.id.to_raw();
    let indicator = Arc::new(UserPresenceGuard::new(state.clone(), user_id.clone()));

    let get_unread_count_ev = {
        let state = state.clone();
        let user_id = user_id.clone();

        move || async move {
            let count = state
                .db
                .discussion_users
                .get_count_of_unread(&user_id)
                .await
                .unwrap_or_default();

            Event::default()
                .event("UnreadDiscussionsCount")
                .data(count.to_string())
        }
    };

    let initial = stream::once({
        let get_unread_count_ev = get_unread_count_ev.clone();
        async move { Ok(get_unread_count_ev().await) }
    });

    let rx = state.event_sender.subscribe();
    let broadcast_stream = BroadcastStream::new(rx).filter_map(move |msg| {
        let indicator = indicator.clone();
        let user_id = user_id.clone();
        let get_unread_count_ev = get_unread_count_ev.clone();

        async move {
            let _tracker = indicator.clone();
            match msg {
                Err(_) => None,
                Ok(msg) => match msg.event {
                    AppEventType::UserNotificationEvent(data)
                        if msg.receivers.contains(&user_id) =>
                    {
                        Some(Ok(Event::default()
                            .event("Notifications")
                            .data(json!(data).to_string())))
                    }
                    AppEventType::UserStatus(data) => {
                        Some(Ok(Event::default().event("UserStatus").data(
                            json!({ "is_online": data.is_online , "user_id": msg.user_id.clone() })
                                .to_string(),
                        )))
                    }

                    AppEventType::UpdateDiscussionsUsers(_users)
                        if msg.receivers.contains(&user_id) =>
                    {
                        Some(Ok(get_unread_count_ev().await))
                    }

                    AppEventType::UpdatedUserBalance if msg.receivers.contains(&user_id) => {
                        Some(Ok(Event::default().event("UpdatedUserBalance")))
                    }

                    _ => None,
                },
            }
        }
    });

    let stream = initial.chain(broadcast_stream);
    Ok(Sse::new(stream).keep_alive(KeepAlive::default()))
}
