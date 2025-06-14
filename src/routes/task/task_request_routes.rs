use crate::entities::task::{task_request_entity, task_request_participation_entity};
use crate::entities::user_auth::{local_user_entity, user_notification_entity};
use crate::entities::wallet::{lock_transaction_entity, wallet_entity};
use crate::middleware;
use crate::services::notification_service::NotificationService;
use askama_axum::Template;
use axum::extract::{Path, State};
use axum::response::Html;
use axum::routing::{get, post};
use axum::{Json, Router};
use axum_typed_multipart::{TryFromMultipart, TypedMultipart};
use chrono::{DateTime, Duration, Utc};
use local_user_entity::LocalUserDbService;
use lock_transaction_entity::{LockTransactionDbService, UnlockTrigger};
use middleware::ctx::Ctx;
use middleware::error::{AppError, CtxResult};
use middleware::mw_ctx::CtxState;
use middleware::utils::db_utils::{record_exists, IdentIdName, ViewFieldSelector};
use middleware::utils::extractor_utils::JsonOrFormValidated;
use middleware::utils::request_utils::CreatedResponse;
use middleware::utils::string_utils::get_string_thing;
use serde::{Deserialize, Serialize};
use serde_json::from_str;
use std::fmt::{Display, Formatter};
use std::sync::Arc;
use surrealdb::sql::{Id, Thing};
use task_request_entity::{
    DeliverableType, RewardType, TaskRequest, TaskRequestDbService, TaskStatus, UserTaskRole,
    TABLE_NAME,
};
use task_request_participation_entity::{TaskParticipationDbService, TaskRequestParticipantion};
use user_notification_entity::{
    UserNotification, UserNotificationDbService, UserNotificationEvent,
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
    #[validate(length(min = 5, message = "Min 5 characters for to_user"))]
    pub to_user: String,
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
    pub post_id: Option<String>,
}

#[derive(Deserialize, Serialize, Debug)]
pub struct TaskRequestView {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<Thing>,
    pub from_user: UserView,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub to_user: Option<UserView>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub request_post: Option<Thing>,
    pub request_txt: String,
    pub participants: Vec<TaskRequestParticipationView>,
    pub status: String,
    pub reward_type: RewardType,
    pub valid_until: DateTime<Utc>,
    pub currency: CurrencySymbol,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub deliverables: Option<Vec<DeliverableView>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub r_created: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub r_updated: Option<String>,
}

impl ViewFieldSelector for TaskRequestView {
    fn get_select_query_fields(_ident: &IdentIdName) -> String {
        "id, from_user.{id, username, full_name} as from_user, to_user.{id, username, full_name} as to_user, on_post, request_txt, reward_type, valid_until, currency, participants.*.{id, user.{id, username, full_name}, amount, currency} as participants, status, deliverables.*.{id, urls, post, user.{id, username, full_name}}, r_created, r_updated".to_string()
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

#[derive(Template, Serialize, Deserialize, Debug, Clone)]
#[template(path = "nera2/default-content.html")]
pub struct DeliverableView {
    pub id: Thing,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub urls: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub post: Option<Thing>,
    pub user: UserView,
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
async fn user_requests_received(State(state): State<Arc<CtxState>>, ctx: Ctx) -> CtxResult<String> {
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
    .user_post_list_view::<TaskRequestView>(UserTaskRole::ToUser, to_user, None, None)
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
    .user_post_list_view::<TaskRequestView>(UserTaskRole::FromUser, from_user, None, None)
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
//     .await?;

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
    let from_user = LocalUserDbService {
        db: &state.db.client,
        ctx: &ctx,
    }
    .get_ctx_user_thing()
    .await?;

    let to_user = if t_request_input.to_user.len() > 0 {
        get_string_thing(t_request_input.to_user)?
    } else {
        return Err(ctx.to_ctx_error(AppError::Generic {
            description: "to_user must have value".to_string(),
        }));
    };

    let content = if t_request_input.content.len() > 0 {
        t_request_input.content
    } else {
        return Err(ctx.to_ctx_error(AppError::Generic {
            description: "content must have value".to_string(),
        }));
    };

    let offer_amount = t_request_input.offer_amount.unwrap_or(0);
    let offer_currency = CurrencySymbol::USD;
    let valid_until = Utc::now().checked_add_signed(Duration::days(5)).unwrap();

    let post_id = t_request_input.post_id.unwrap_or("".to_string());
    let post = if post_id.len() > 0 {
        Some(get_string_thing(post_id)?)
    } else {
        None
    };

    // TODO in db transaction
    let lock = if offer_amount > 0 {
        let lock_service = LockTransactionDbService {
            db: &state.db.client,
            ctx: &ctx,
        };
        Some(
            lock_service
                .lock_user_asset_tx(
                    &from_user,
                    offer_amount,
                    offer_currency.clone(),
                    vec![UnlockTrigger::Timestamp {
                        at: valid_until.clone(),
                    }],
                )
                .await?,
        )
    } else {
        None
    };
    let t_req_id = Thing::from((TABLE_NAME, Id::ulid()));
    let participant = TaskParticipationDbService {
        db: &state.db.client,
        ctx: &ctx,
    }
    .create_update(TaskRequestParticipantion {
        id: None,
        amount: offer_amount,
        user: from_user.clone(),
        lock,
        currency: offer_currency.clone(),
        votes: None,
    })
    .await?;
    let t_request = TaskRequestDbService {
        db: &state.db.client,
        ctx: &ctx,
    }
    .create(TaskRequest {
        id: Some(t_req_id.clone()),
        from_user: from_user.clone(),
        to_user: Some(to_user.clone()),
        on_post: post,
        request_txt: content,
        deliverable_type: DeliverableType::PublicPost,
        participants: vec![participant.id.unwrap()],
        status: TaskStatus::Requested.to_string(),
        reward_type: RewardType::OnDelivery,
        valid_until,
        currency: offer_currency,
        deliverables: None,
        r_created: None,
        r_updated: None,
    })
    .await?;

    let n_service = NotificationService::new(&state.db.client, &ctx, &state.event_sender);

    let _ = n_service
        .on_update_balance(&from_user.clone(), &vec![from_user.clone()])
        .await;

    let _ = n_service
        .on_created_task(&from_user, &t_req_id, &to_user)
        .await?;

    let _ = n_service
        .on_received_task(&from_user, &t_req_id, &to_user)
        .await?;

    ctx.to_htmx_or_json(CreatedResponse {
        id: t_request.id.unwrap().to_raw(),
        uri: None,
        success: true,
    })
}

async fn accept_task_request(
    State(state): State<Arc<CtxState>>,
    ctx: Ctx,
    Path(task_id): Path<String>,
    Json(t_request_input): Json<AcceptTaskRequestInput>,
) -> CtxResult<Html<String>> {
    let to_user = LocalUserDbService {
        db: &state.db.client,
        ctx: &ctx,
    }
    .get_ctx_user_thing()
    .await?;
    let task_id = get_string_thing(task_id)?;
    let status = match t_request_input.accept {
        true => TaskStatus::Accepted,
        false => TaskStatus::Rejected,
    };
    TaskRequestDbService {
        db: &state.db.client,
        ctx: &ctx,
    }
    .update_status_received_by_user(to_user, task_id.clone(), status, None, None)
    .await?;

    ctx.to_htmx_or_json(CreatedResponse {
        id: task_id.to_raw(),
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

    let task_id = get_string_thing(task_id)?;
    let task_req_ser = TaskRequestDbService {
        db: &state.db.client,
        ctx: &ctx,
    };

    let task = task_req_ser.get(IdentIdName::Id(task_id.clone())).await?;

    if task.to_user.is_some() && task.to_user != Some(delivered_by.clone()) {
        return Err(ctx.to_ctx_error(AppError::AuthorizationFail {
            required: "This user was not requested to deliver".to_string(),
        }));
    }

    let (deliverables, post) = match task.deliverable_type {
        DeliverableType::PublicPost => {
            let post_id = get_string_thing(t_request_input.post_id.ok_or(AppError::Generic {
                description: "Missing post_id".to_string(),
            })?)?;
            record_exists(&state.db.client, &post_id)
                .await
                .map_err(|e| ctx.to_ctx_error(e))?;
            (None, Some(post_id))
        } /*DeliverableType::Participants => {
              let file_data = t_request_input.file_1.unwrap();
              let file_name = file_data.metadata.file_name.unwrap();
              let ext = file_name.split(".").last().ok_or(AppError::Generic {
                  description: "File has no extension".to_string(),
              })?;

              let file_name: String = TaskDeliverableFileName {
                  task_id: task_id.clone(),
                  file_nr: 1,
                  ext: ext.to_string(),
              }
                  .to_string();
              let path = FPath::new(&uploads_dir).join(file_name.clone());
              file_data
                  .contents
                  .persist(path.clone())
                  .map_err(|e| {
                      ctx.to_ctx_error(AppError::Generic {
                          description: "Upload failed".to_string(),
                      })
                  })?;
              let file_uri = format!("{DELIVERIES_URL_BASE}/{file_name}");

              let deliverables = vec![file_uri];
              (deliverables, None)
          }*/
    };

    let (task, deliverable_id) = task_req_ser
        .update_status_received_by_user(
            delivered_by.clone(),
            task_id.clone(),
            TaskStatus::Delivered,
            deliverables.clone(),
            post,
        )
        .await?;
    let deliverable_id = deliverable_id.ok_or(AppError::EntityFailIdNotFound {
        ident: "deliverable_id not created".to_string(),
    })?;

    let participations_service = TaskParticipationDbService {
        db: &state.db.client,
        ctx: &ctx,
    };

    let notify_task_participant_ids: Vec<Thing> = TaskParticipationDbService {
        db: &state.db.client,
        ctx: &ctx,
    }
    .get_ids(&task.participants)
    .await?
    .into_iter()
    .map(|t| t.user)
    .collect();

    let n_service = NotificationService::new(&state.db.client, &ctx, &state.event_sender);

    match task.reward_type {
        RewardType::OnDelivery => {
            participations_service
                .process_payments(task.to_user.as_ref().unwrap(), task.participants.clone())
                .await?;
            n_service
                .on_update_balance(&delivered_by, &notify_task_participant_ids)
                .await?;
        } /*RewardType::VoteWinner{..} => {
              // add action for this reward type
          }*/
    }

    n_service
        .on_deliver_task(
            &delivered_by,
            task.id.unwrap(),
            deliverable_id,
            &notify_task_participant_ids,
        )
        .await?;

    ctx.to_htmx_or_json(CreatedResponse {
        id: task_id.to_raw(),
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

async fn participate_task_request_offer(
    State(state): State<Arc<CtxState>>,
    ctx: Ctx,
    Path(task_offer_id): Path<String>,
    JsonOrFormValidated(t_request_offer_input): JsonOrFormValidated<TaskRequestOfferInput>,
) -> CtxResult<Html<String>> {
    let from_user = LocalUserDbService {
        db: &state.db.client,
        ctx: &ctx,
    }
    .get_ctx_user_thing()
    .await?;
    let task_request_db_service = TaskRequestDbService {
        db: &state.db.client,
        ctx: &ctx,
    };

    let task_offer = task_request_db_service
        .add_participation(
            get_string_thing(task_offer_id)?,
            from_user.clone(),
            t_request_offer_input.amount,
        )
        .await?;

    let _notif_res = UserNotificationDbService {
        db: &state.db.client,
        ctx: &ctx,
    }
    .create(UserNotification {
        id: None,
        user: from_user,
        event: UserNotificationEvent::UserBalanceUpdate,
        content: "".to_string(),
        r_created: None,
    })
    .await;

    ctx.to_htmx_or_json(CreatedResponse {
        success: true,
        id: task_offer.id.unwrap().to_raw(),
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
