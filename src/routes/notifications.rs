use axum::extract::{Path, Query, State};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::Deserialize;
use std::sync::Arc;

use local_user_entity::LocalUserDbService;
use middleware::ctx::Ctx;
use middleware::error::CtxResult;

use crate::entities::user_auth::local_user_entity;
use crate::entities::user_notification::UserNotification;
use crate::interfaces::repositories::user_notifications::{
    GetNotificationOptions, UserNotificationsInterface,
};
use crate::middleware;
use crate::middleware::mw_ctx::CtxState;
use crate::middleware::utils::db_utils::QryOrder;

pub fn routes() -> Router<Arc<CtxState>> {
    Router::new()
        .route("/api/notifications", get(get_notifications))
        .route("/api/notifications/read", post(read_all))
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
        .read(&notification_id, &user.to_raw())
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
    start: Option<u32>,
    count: Option<u8>,
    is_read: Option<bool>,
    order_dir: Option<QryOrder>,
}

async fn get_notifications(
    State(state): State<Arc<CtxState>>,
    ctx: Ctx,
    Query(query): Query<GetNotificationsQuery>,
) -> CtxResult<Json<Vec<UserNotification>>> {
    let user = LocalUserDbService {
        db: &state.db.client,
        ctx: &ctx,
    }
    .get_ctx_user_thing()
    .await?;

    let notifications = state
        .db
        .user_notifications
        .get_by_user(
            &user.to_raw(),
            GetNotificationOptions {
                limit: query.count.unwrap_or(50),
                start: query.start.unwrap_or(0),
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
    ctx: Ctx,
    Query(query): Query<GetCountQuery>,
) -> CtxResult<Json<u64>> {
    let user = LocalUserDbService {
        db: &state.db.client,
        ctx: &ctx,
    }
    .get_ctx_user_thing()
    .await?;

    let count = state
        .db
        .user_notifications
        .get_count(&user.to_raw(), query.is_read)
        .await?;

    Ok(Json(count))
}
