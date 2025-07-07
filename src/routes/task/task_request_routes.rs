use crate::entities::community::post_entity::PostDbService;
use crate::entities::task::task_request_entity;
use crate::entities::task::task_request_entity::TaskRequestType;
use crate::entities::task::task_request_participation_entity::TaskRequestParticipation;
use crate::entities::task_request_user::{
    TaskRequestUser, TaskRequestUserResult, TaskRequestUserStatus,
};
use crate::entities::user_auth::local_user_entity;
use crate::entities::user_notification::UserNotificationEvent;
use crate::entities::wallet::{lock_transaction_entity, wallet_entity};
use crate::interfaces::repositories::task_participators::TaskParticipatorsRepositoryInterface;
use crate::interfaces::repositories::task_request_users::TaskRequestUsersRepositoryInterface;
use crate::interfaces::repositories::user_notifications::UserNotificationsInterface;
use crate::middleware;
use crate::middleware::utils::string_utils::get_str_thing;
use crate::services::notification_service::NotificationService;
use askama_axum::Template;
use axum::extract::{Path, Query, State};
use axum::response::Html;
use axum::routing::{get, post};
use axum::Router;
use axum_typed_multipart::{TryFromMultipart, TypedMultipart};
use chrono::{Duration, Utc};
use local_user_entity::LocalUserDbService;
use lock_transaction_entity::{LockTransactionDbService, UnlockTrigger};
use middleware::ctx::Ctx;
use middleware::error::{AppError, CtxResult};
use middleware::mw_ctx::CtxState;
use middleware::utils::db_utils::{IdentIdName, ViewFieldSelector};
use middleware::utils::extractor_utils::JsonOrFormValidated;
use middleware::utils::request_utils::CreatedResponse;
use middleware::utils::string_utils::get_string_thing;
use serde::{Deserialize, Serialize};
use serde_json::from_str;
use std::fmt::{Display, Formatter};
use std::sync::Arc;
use surrealdb::sql::{Id, Thing};
use task_request_entity::{
    DeliverableType, RewardType, TaskRequest, TaskRequestDbService, TABLE_NAME,
};
use validator::Validate;
use wallet_entity::CurrencySymbol;

pub fn routes() -> Router<Arc<CtxState>> {
    Router::new()
        .route("/api/task_request", post(create_entity))
        .route(
            "/api/task_request/list/post/:post_id",
            get(post_task_requests),
        )
        /*.route(
            "/api/task_request/received/post/:post_id",
            get(post_requests_received),
        )*/
        .route("/api/task_request/received", get(user_requests_received))
        /*.route(
            "/api/task_request/given/post/:post_id",
            get(post_requests_given),
        )*/
        .route("/api/task_request/given", get(user_requests_given))
        .route(
            "/api/task_request/:task_id/accept",
            post(accept_task_request),
        )
        .route(
            "/api/task_request/:task_id/reject",
            post(reject_task_request),
        )
        .route(
            "/api/task_request/:task_id/deliver",
            post(deliver_task_request),
        )
        /*.route(
            "/api/task_request/:task_id/offer",
            post(create_task_request_offer),
        )*/
        .route(
            "/api/task_offer/:task_offer_id/participate",
            post(participate_task_request_offer),
        )
    // the file max limit is set on PostInput property
    // .layer(DefaultBodyLimit::max(1024 * 1024 * 30))
}

#[derive(Deserialize, Serialize, Validate)]
pub struct TaskRequestInput {
    #[validate(length(min = 5, message = "Min 5 characters for content"))]
    pub content: String,
    pub to_user: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub post_id: Option<String>,
    pub offer_amount: Option<i64>,
}

#[derive(Deserialize, Serialize, Validate)]
pub struct TaskRequestOfferInput {
    pub amount: i64,
    pub currency: Option<CurrencySymbol>,
}

#[derive(Deserialize, Serialize)]
pub struct AcceptTaskRequestInput {
    pub accept: bool,
}

#[derive(Validate, TryFromMultipart)]
pub struct DeliverTaskRequestInput {
    // currently only use post_id
    // #[form_data(limit = "30MiB")]
    // pub file_1: Option<FieldData<NamedTempFile>>,
    pub post_id: String,
}
#[derive(Deserialize, Serialize, Debug)]
pub struct TaskRequestViewToUsers {
    pub user: UserView,
    pub status: TaskRequestUserStatus,
}
#[derive(Deserialize, Serialize, Debug)]
pub struct TaskRequestView {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<Thing>,
    pub from_user: UserView,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub to_users: Option<Vec<TaskRequestViewToUsers>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub request_post: Option<Thing>,
    pub request_txt: String,
    pub participants: Vec<TaskRequestParticipationView>,
    pub reward_type: RewardType,
    pub currency: CurrencySymbol,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub r_created: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub r_updated: Option<String>,
}

impl ViewFieldSelector for TaskRequestView {
    fn get_select_query_fields(_ident: &IdentIdName) -> String {
        "id, 
        from_user.{id, username, full_name} as from_user,
        on_post, request_txt, reward_type, 
        currency, 
        ->task_request_user.{
            user: out.{id, username, full_name},        
            status
        } as to_users,
        ->task_request_participation.{user:out.{id, username, full_name}, amount, currency} as participants"
            .to_string()
    }
}

#[derive(Template, Serialize, Deserialize, Debug)]
#[template(path = "nera2/default-content.html")]
pub struct TaskRequestParticipationView {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<Thing>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user: Option<UserView>,
    pub currency: CurrencySymbol,
    pub amount: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub r_created: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub r_updated: Option<String>,
}

#[derive(Template, Serialize, Deserialize, Debug, Clone)]
#[template(path = "nera2/default-content.html")]
pub struct UserView {
    pub id: Thing,
    pub username: String,
    pub full_name: Option<String>,
}

/*async fn post_requests_received(
    State(CtxState { _db, .. }): State<CtxState>,
    ctx: Ctx,
    Path(post_id): Path<String>,
) -> CtxResult<String> {
    let to_user = LocalUserDbService {
        db: &_db,
        ctx: &ctx,
    }
    .get_ctx_user_thing()
    .await?;
    let post_id = Some(get_string_thing(post_id)?);

    let list = TaskRequestDbService {
        db: &_db,
        ctx: &ctx,
    }
    .user_post_list_view::<TaskRequestView>(UserTaskRole::ToUser, to_user, post_id, None)
    .await?;
    serde_json::to_string(&list).map_err(|e| ctx.to_ctx_error(e.into()))
}
*/
#[derive(Debug, Deserialize)]
struct GetTaskByToUserQuery {
    status: Option<TaskRequestUserStatus>,
}
async fn user_requests_received(
    State(state): State<Arc<CtxState>>,
    ctx: Ctx,
    Query(query): Query<GetTaskByToUserQuery>,
) -> CtxResult<String> {
    let to_user = LocalUserDbService {
        db: &state.db.client,
        ctx: &ctx,
    }
    .get_ctx_user_thing()
    .await?;

    let list = TaskRequestDbService {
        db: &state.db.client,
        ctx: &ctx,
    }
    .get_by_user::<TaskRequestView>(&to_user, query.status)
    .await?;
    serde_json::to_string(&list).map_err(|e| ctx.to_ctx_error(e.into()))
}

/*async fn post_requests_given(
    State(CtxState { _db, .. }): State<CtxState>,
    ctx: Ctx,
    Path(post_id): Path<String>,
) -> CtxResult<String> {
    let from_user = LocalUserDbService {
        db: &_db,
        ctx: &ctx,
    }
    .get_ctx_user_thing()
    .await?;
    let post_id = Some(get_string_thing(post_id)?);

    let list = TaskRequestDbService {
        db: &_db,
        ctx: &ctx,
    }
    .user_post_list_view::<TaskRequestView>(UserTaskRole::FromUser, from_user, post_id, None)
    .await?;
    serde_json::to_string(&list).map_err(|e| ctx.to_ctx_error(e.into()))
}
*/
async fn user_requests_given(State(state): State<Arc<CtxState>>, ctx: Ctx) -> CtxResult<String> {
    let from_user = LocalUserDbService {
        db: &state.db.client,
        ctx: &ctx,
    }
    .get_ctx_user_thing()
    .await?;

    let list = TaskRequestDbService {
        db: &state.db.client,
        ctx: &ctx,
    }
    .get_by_creator::<TaskRequestView>(from_user, None, None)
    .await?;
    serde_json::to_string(&list).map_err(|e| ctx.to_ctx_error(e.into()))
}

async fn post_task_requests(
    State(state): State<Arc<CtxState>>,
    ctx: Ctx,
    Path(post_id): Path<String>,
) -> CtxResult<String> {
    let list = TaskRequestDbService {
        db: &state.db.client,
        ctx: &ctx,
    }
    .on_post_list_view::<TaskRequestView>(get_string_thing(post_id)?)
    .await?;
    serde_json::to_string(&list).map_err(|e| ctx.to_ctx_error(e.into()))
}

// this is not used anywhere. so commenting it for now, might need later - @anukulpandey 31/03/2025
// async fn user_task_requests_accepted(
//     State(CtxState { _db, .. }): State<CtxState>,
//     ctx: Ctx,
// ) -> CtxResult<String> {
//     let from_user = LocalUserDbService {
//         db: &_db,
//         ctx: &ctx,
//     }
//     .get_ctx_user_thing()
//    .await?;
//     let list = TaskRequestDbService {
//         db: &_db,
//         ctx: &ctx,
//     }
//     .user_status_list(UserTaskRole::FromUser, from_user, TaskStatus::Accepted)
//     .await?;
//     serde_json::to_string(&list).map_err(|e| ctx.to_ctx_error(e.into()))
// }

// this is not used anywhere. so commenting it for now, might need later - @anukulpandey 31/03/2025
// async fn user_task_requests_delivered(
//     State(CtxState { _db, .. }): State<CtxState>,
//     ctx: Ctx,
// ) -> CtxResult<String> {
//     let from_user = LocalUserDbService {
//         db: &_db,
//         ctx: &ctx,
//     }
//     .get_ctx_user_thing()
//     .await?;

//     let list = TaskRequestDbService {
//         db: &_db,
//         ctx: &ctx,
//     }
//     .user_status_list(UserTaskRole::FromUser, from_user, TaskStatus::Delivered)
//     .await?;
//     serde_json::to_string(&list).map_err(|e| ctx.to_ctx_error(e.into()))
// }

async fn create_entity(
    State(state): State<Arc<CtxState>>,
    ctx: Ctx,
    JsonOrFormValidated(t_request_input): JsonOrFormValidated<TaskRequestInput>,
) -> CtxResult<Html<String>> {
    let user_db_service = LocalUserDbService {
        db: &state.db.client,
        ctx: &ctx,
    };
    let from_user = user_db_service.get_ctx_user_thing().await?;

    let (to_user, task_type) = if let Some(to_user) = t_request_input.to_user {
        let to_user_thing = get_str_thing(&to_user)?;
        (
            Some(user_db_service.get(IdentIdName::Id(to_user_thing)).await?),
            TaskRequestType::Close,
        )
    } else {
        (None, TaskRequestType::Open)
    };

    let offer_amount = t_request_input.offer_amount.unwrap_or(0);
    let offer_currency = CurrencySymbol::USD;
    let valid_until = Utc::now().checked_add_signed(Duration::days(5)).unwrap();

    let post = if let Some(ref post_id) = t_request_input.post_id {
        let post_thing = get_string_thing(post_id.clone())?;
        let post_db_service = PostDbService {
            db: &state.db.client,
            ctx: &ctx,
        };
        post_db_service
            .must_exist(IdentIdName::Id(post_thing.clone()))
            .await?;
        Some(post_thing)
    } else {
        None
    };

    let t_req_id = Thing::from((TABLE_NAME, Id::ulid()));

    let t_request = TaskRequestDbService {
        db: &state.db.client,
        ctx: &ctx,
    }
    .create(TaskRequest {
        id: Some(t_req_id.clone()),
        from_user: from_user.clone(),
        on_post: post,
        r#type: task_type,
        request_txt: t_request_input.content,
        deliverable_type: DeliverableType::PublicPost,
        reward_type: RewardType::OnDelivery,
        currency: offer_currency.clone(),
        deliverables: None,
        r_created: None,
        r_updated: None,
    })
    .await?;

    if offer_amount > 0 {
        let tx_db_service = LockTransactionDbService {
            db: &state.db.client,
            ctx: &ctx,
        };
        let tx_id = tx_db_service
            .lock_user_asset_tx(
                &from_user,
                offer_amount,
                offer_currency.clone(),
                vec![UnlockTrigger::Timestamp {
                    at: valid_until.clone(),
                }],
            )
            .await?;
        let _ = state
            .db
            .task_participators
            .create(
                &t_request.id.as_ref().unwrap().id.to_raw(),
                &from_user.id.to_raw(),
                &tx_id.id.to_raw(),
                offer_amount as u64,
                &offer_currency.to_string(),
            )
            .await
            .map_err(|e| AppError::SurrealDb {
                source: e.to_string(),
            })?;
    }

    if let Some(ref user) = to_user {
        let _ = state
            .db
            .task_request_users
            .create(
                &t_request.id.as_ref().unwrap().id.to_raw(),
                &user.id.as_ref().unwrap().id.to_raw(),
                TaskRequestUserStatus::Requested.as_str(),
            )
            .await
            .map_err(|e| AppError::SurrealDb {
                source: e.to_string(),
            })?;
    };

    let n_service = NotificationService::new(
        &state.db.client,
        &ctx,
        &state.event_sender,
        &state.db.user_notifications,
    );

    let _ = n_service
        .on_update_balance(&from_user.clone(), &vec![from_user.clone()])
        .await;

    if let Some(ref user) = to_user {
        let _ = n_service
            .on_created_task(&from_user, &t_req_id, &user.id.as_ref().unwrap())
            .await?;

        let _ = n_service
            .on_received_task(&from_user, &t_req_id, &user.id.as_ref().unwrap())
            .await?;
    };

    ctx.to_htmx_or_json(CreatedResponse {
        id: t_request.id.unwrap().to_raw(),
        uri: None,
        success: true,
    })
}

#[derive(Deserialize, Serialize, Debug)]
pub struct TaskRequestForToUsers {
    pub id: Thing,
    pub reward_type: RewardType,
    pub r#type: TaskRequestType,
    // pub participant_ids: Vec<Thing>,
    pub participants: Vec<TaskRequestParticipation>,
    pub to_users: Vec<TaskRequestUser>,
}

impl ViewFieldSelector for TaskRequestForToUsers {
    fn get_select_query_fields(_ident: &IdentIdName) -> String {
        "id, 
        reward_type,
        ->task_request_participation.*.{id, amount, currency, lock, user: out} as participants,
        ->task_request_participation.*.out as participant_ids,
        ->task_request_user.{id:record::id(id),task:record::id(in),user:record::id(out),status, result} as to_users,
        type"
            .to_string()
    }
}

async fn reject_task_request(
    State(state): State<Arc<CtxState>>,
    ctx: Ctx,
    Path(task_id): Path<String>,
) -> CtxResult<Html<String>> {
    let user_id = LocalUserDbService {
        db: &state.db.client,
        ctx: &ctx,
    }
    .get_ctx_user_thing()
    .await?;

    let task_db_service = TaskRequestDbService {
        db: &state.db.client,
        ctx: &ctx,
    };
    let task_thing = get_str_thing(&task_id)?;
    let task = task_db_service
        .get_by_id::<TaskRequestForToUsers>(&task_thing)
        .await?;
    let user_id_id = user_id.id.to_raw();
    let task_user = task.to_users.iter().find(|v| v.user == user_id_id);

    let allow = task_user.map_or(false, |v| {
        v.status == TaskRequestUserStatus::Requested || v.status == TaskRequestUserStatus::Accepted
    });

    if !allow {
        return Err(AppError::Generic {
            description: "Forbidden".to_string(),
        }
        .into());
    }

    state
        .db
        .task_request_users
        .update(
            &task_user.as_ref().unwrap().id,
            TaskRequestUserStatus::Rejected.as_str(),
            None,
        )
        .await
        .map_err(|_| AppError::SurrealDb {
            source: format!("reject_task"),
        })?;

    ctx.to_htmx_or_json(CreatedResponse {
        id: task_id,
        uri: None,
        success: true,
    })
}

async fn accept_task_request(
    State(state): State<Arc<CtxState>>,
    ctx: Ctx,
    Path(task_id): Path<String>,
) -> CtxResult<Html<String>> {
    let user_id = LocalUserDbService {
        db: &state.db.client,
        ctx: &ctx,
    }
    .get_ctx_user_thing()
    .await?;

    let task_db_service = TaskRequestDbService {
        db: &state.db.client,
        ctx: &ctx,
    };

    let task_thing = get_str_thing(&task_id)?;
    let task = task_db_service
        .get_by_id::<TaskRequestForToUsers>(&task_thing)
        .await?;

    if task.participants.iter().any(|t| t.user == user_id) {
        return Err(AppError::Generic {
            description: "Forbidden".to_string(),
        }
        .into());
    }

    let user_id_id = user_id.id.to_raw();
    let task_user = task.to_users.iter().find(|v| v.user == user_id_id);

    match task.r#type {
        TaskRequestType::Open => {
            if task_user.is_some() {
                return Err(AppError::Generic {
                    description: "Forbidden".to_string(),
                }
                .into());
            };

            let _ = state
                .db
                .task_request_users
                .create(
                    &task.id.id.to_raw(),
                    &user_id_id,
                    TaskRequestUserStatus::Accepted.as_str(),
                )
                .await
                .map_err(|e| AppError::SurrealDb {
                    source: e.to_string(),
                })?;
        }
        TaskRequestType::Close => {
            if task_user.map_or(true, |v| v.status != TaskRequestUserStatus::Requested) {
                return Err(AppError::Generic {
                    description: "Forbidden".to_string(),
                }
                .into());
            }

            let _ = state
                .db
                .task_request_users
                .update(
                    &task_user.as_ref().unwrap().id,
                    TaskRequestUserStatus::Accepted.as_str(),
                    None,
                )
                .await
                .map_err(|e| AppError::SurrealDb {
                    source: e.to_string(),
                })?;
        }
    }

    ctx.to_htmx_or_json(CreatedResponse {
        id: task_id,
        uri: None,
        success: true,
    })
}

async fn deliver_task_request(
    State(state): State<Arc<CtxState>>,
    ctx: Ctx,
    Path(task_id): Path<String>,
    TypedMultipart(t_request_input): TypedMultipart<DeliverTaskRequestInput>,
) -> CtxResult<Html<String>> {
    let delivered_by = LocalUserDbService {
        db: &state.db.client,
        ctx: &ctx,
    }
    .get_ctx_user_thing()
    .await?;

    let task_db_service = TaskRequestDbService {
        db: &state.db.client,
        ctx: &ctx,
    };

    let task_thing = get_str_thing(&task_id)?;
    let task = task_db_service
        .get_by_id::<TaskRequestForToUsers>(&task_thing)
        .await?;

    let user_id_id = delivered_by.id.to_raw();
    let task_user = task
        .to_users
        .iter()
        .find(|v| v.user == user_id_id && v.status == TaskRequestUserStatus::Accepted);

    if task_user.is_none() {
        return Err(AppError::Generic {
            description: "Forbidden".to_string(),
        }
        .into());
    }

    let post_db_service = PostDbService {
        db: &state.db.client,
        ctx: &ctx,
    };
    let post_thing = get_str_thing(&t_request_input.post_id)?;
    post_db_service
        .must_exist(IdentIdName::Id(post_thing))
        .await?;

    state
        .db
        .task_request_users
        .update(
            &task_user.unwrap().id,
            TaskRequestUserStatus::Delivered.as_str(),
            Some(TaskRequestUserResult {
                urls: None,
                post: Some(t_request_input.post_id),
            }),
        )
        .await
        .map_err(|_| AppError::SurrealDb {
            source: "deliver_task".to_string(),
        })?;

    let n_service = NotificationService::new(
        &state.db.client,
        &ctx,
        &state.event_sender,
        &state.db.user_notifications,
    );

    let participant_ids = task
        .participants
        .iter()
        .map(|t| t.user.clone())
        .collect::<Vec<Thing>>();

    // TODO payment should be when task is over by the time
    match task.reward_type {
        RewardType::OnDelivery => {
            let lock_service = LockTransactionDbService {
                db: &state.db.client,
                ctx: &ctx,
            };

            for participant in &task.participants {
                if let Some(ref lock) = participant.lock {
                    let _ = lock_service
                        .process_locked_payment(lock, &delivered_by)
                        .await;
                }
            }

            n_service
                .on_update_balance(&delivered_by, &participant_ids)
                .await?;
        } /*RewardType::VoteWinner{..} => {
              // add action for this reward type
          }*/
    }

    n_service
        .on_deliver_task(&delivered_by, task_thing.clone(), &participant_ids)
        .await?;

    ctx.to_htmx_or_json(CreatedResponse {
        id: task_id,
        uri: None,
        success: true,
    })
}

/*async fn create_task_request_offer(
    State(CtxState { _db, .. }): State<CtxState>,
    ctx: Ctx,
    Path(task_id): Path<String>,
    JsonOrFormValidated(t_request_offer_input): JsonOrFormValidated<TaskRequestOfferInput>,
) -> CtxResult<Html<String>> {
    let from_user = LocalUserDbService {
        db: &_db,
        ctx: &ctx,
    }
    .get_ctx_user_thing()
    .await?;
    let task_offer = TaskRequestOfferDbService {
        db: &_db,
        ctx: &ctx,
    }
    .create_task_offer(
        get_string_thing(task_id)?,
        from_user,
        t_request_offer_input.amount,
    )
    .await?;

    ctx.to_htmx_or_json(CreatedResponse {
        success: true,
        id: task_offer.id.unwrap().to_raw(),
        uri: None,
    })
}
*/

#[derive(Deserialize, Serialize, Debug)]
pub struct TaskRequestAddParticipators {
    pub id: Thing,
    pub reward_type: RewardType,
    pub r#type: TaskRequestType,
    pub participants: Vec<TaskRequestParticipation>,
    pub to_users: Vec<TaskRequestUser>,
}

impl ViewFieldSelector for TaskRequestAddParticipators {
    fn get_select_query_fields(_ident: &IdentIdName) -> String {
        "id, 
        reward_type,
        ->task_request_participation.*.{id, amount, currency, lock, user: out} as participants,
        ->task_request_user.{id:record::id(id),task:record::id(in),user:record::id(out),status} as to_users,
        type"
            .to_string()
    }
}
async fn participate_task_request_offer(
    State(state): State<Arc<CtxState>>,
    ctx: Ctx,
    Path(task_id): Path<String>,
    JsonOrFormValidated(data): JsonOrFormValidated<TaskRequestOfferInput>,
) -> CtxResult<Html<String>> {
    let current_user = LocalUserDbService {
        db: &state.db.client,
        ctx: &ctx,
    }
    .get_ctx_user_thing()
    .await?;

    let task_db_service = TaskRequestDbService {
        db: &state.db.client,
        ctx: &ctx,
    };

    if data.amount <= 0 {
        return Err(AppError::Generic {
            description: "Forbidden".to_string(),
        }
        .into());
    }

    let task_thing = get_str_thing(&task_id)?;
    let task = task_db_service
        .get_by_id::<TaskRequestAddParticipators>(&task_thing)
        .await?;
    let is_some_accepted_or_delivered = task.to_users.iter().any(|v| {
        v.status == TaskRequestUserStatus::Accepted || v.status == TaskRequestUserStatus::Delivered
    });
    let offer_currency = data.currency.unwrap_or(CurrencySymbol::USD);

    if is_some_accepted_or_delivered {
        return Err(AppError::Generic {
            description: "Forbidden".to_string(),
        }
        .into());
    }

    let participant = task.participants.iter().find(|p| p.user == current_user);
    let tx_db_service = LockTransactionDbService {
        db: &state.db.client,
        ctx: &ctx,
    };

    let offer_id = match participant {
        Some(p) => {
            if let Some(ref lock) = p.lock {
                tx_db_service.unlock_user_asset_tx(lock).await?;
            }
            let tx_id = tx_db_service
                .lock_user_asset_tx(&current_user, data.amount, offer_currency.clone(), vec![])
                .await?;
            let _ = state
                .db
                .task_participators
                .update(
                    &p.id.as_ref().unwrap().id.to_raw(),
                    &tx_id.id.to_raw(),
                    data.amount as u64,
                    &offer_currency.to_string(),
                )
                .await
                .map_err(|e| AppError::SurrealDb {
                    source: e.to_string(),
                })?;
            p.id.as_ref().unwrap().to_raw()
        }
        None => {
            let tx_id = tx_db_service
                .lock_user_asset_tx(&current_user, data.amount, offer_currency.clone(), vec![])
                .await?;
            let id = state
                .db
                .task_participators
                .create(
                    &task_thing.id.to_raw(),
                    &current_user.id.to_raw(),
                    &tx_id.id.to_raw(),
                    data.amount as u64,
                    &offer_currency.to_string(),
                )
                .await
                .map_err(|e| AppError::SurrealDb {
                    source: e.to_string(),
                })?;
            id
        }
    };

    state
        .db
        .user_notifications
        .create(
            &current_user.to_raw(),
            "participate task",
            UserNotificationEvent::UserBalanceUpdate.as_str(),
            &vec![current_user.to_raw()],
            None,
        )
        .await?;

    ctx.to_htmx_or_json(CreatedResponse {
        success: true,
        id: offer_id,
        uri: None,
    })
}

struct TaskDeliverableFileName {
    task_id: Thing,
    ext: String,
    file_nr: i8,
}

impl Display for TaskDeliverableFileName {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(
            format!(
                "tid_{}-file_{}.{}",
                self.task_id.id.to_raw(),
                self.file_nr,
                self.ext
            )
            .as_str(),
        )
    }
}

impl TryFrom<String> for TaskDeliverableFileName {
    type Error = AppError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        let error = AppError::Generic {
            description: "Can not parse task file".to_string(),
        };
        let tid_fname = value.split_once("-").ok_or(error.clone())?;
        let tid = tid_fname.0.split_once("_").ok_or(error.clone())?.1;
        let task_id = Thing::from((task_request_entity::TABLE_NAME, tid));

        let fnr_ext = tid_fname.1.split_once(".").ok_or(error.clone())?;
        let fnr = fnr_ext.0.split_once("_").ok_or(error.clone())?;
        let file_nr: i8 = from_str(fnr.1).map_err(|_| error.clone())?;
        let ext = fnr_ext.1.to_string();

        Ok(TaskDeliverableFileName {
            ext,
            file_nr,
            task_id,
        })
    }
}
