use askama_axum::Template;
use axum::extract::State;
use axum::response::Html;
use axum::routing::get;
use axum::Router;
use serde::{Deserialize, Serialize};
use std::clone::Clone;
use std::string::ToString;
use surrealdb::sql::Thing;

use local_user_entity::LocalUserDbService;
use middleware::ctx::Ctx;
use middleware::error::{AppError, CtxResult};
use middleware::mw_ctx::CtxState;
use middleware::utils::db_utils::{IdentIdName, ViewFieldSelector};
use middleware::utils::extractor_utils::DiscussionParams;
use user_notification_entity::{UserNotificationDbService, UserNotificationEvent};

use crate::entities::user_auth::{local_user_entity, user_notification_entity};
use crate::middleware;

pub fn routes(state: CtxState) -> Router {
    Router::new()
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
