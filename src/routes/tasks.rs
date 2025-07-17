use crate::entities::task::task_request_entity::{self};
use crate::entities::task_request_user::TaskParticipantStatus;
use crate::entities::user_auth::local_user_entity;
use crate::entities::wallet::wallet_entity;
use crate::middleware;
use crate::middleware::utils::db_utils::ViewRelateField;
use crate::models::web::{TaskRequestDonorView, UserView};
use crate::services::task_service::{TaskDeliveryData, TaskDonorData, TaskService};
use axum::extract::{Path, Query, State};
use axum::response::Html;
use axum::routing::{get, post};
use axum::{Json, Router};
use chrono::{DateTime, Utc};
use local_user_entity::LocalUserDbService;
use middleware::ctx::Ctx;
use middleware::error::CtxResult;
use middleware::mw_ctx::CtxState;
use middleware::utils::db_utils::ViewFieldSelector;
use middleware::utils::extractor_utils::JsonOrFormValidated;
use middleware::utils::request_utils::CreatedResponse;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use surrealdb::sql::Thing;
use task_request_entity::{RewardType, TaskRequestDbService};
use validator::Validate;
use wallet_entity::CurrencySymbol;

pub fn routes() -> Router<Arc<CtxState>> {
    Router::new()
        .route("/api/tasks/received", get(user_requests_received))
        .route("/api/tasks/given", get(user_requests_given))
        .route("/api/tasks/:task_id/accept", post(accept_task_request))
        .route("/api/tasks/:task_id/reject", post(reject_task_request))
        .route("/api/tasks/:task_id/deliver", post(deliver_task_request))
        .route("/api/tasks/:task_id/donor", post(upsert_donor))
}

#[derive(Deserialize, Serialize, Validate)]
pub struct TaskRequestOfferInput {
    pub amount: u64,
    pub currency: Option<CurrencySymbol>,
}

#[derive(Validate, Deserialize)]
pub struct DeliverTaskRequestInput {
    pub post_id: String,
}

#[derive(Deserialize, Serialize, Debug)]
pub struct TaskRequestViewParticipant {
    pub user: UserView,
    pub status: TaskParticipantStatus,
}
#[derive(Deserialize, Serialize, Debug)]
pub struct TaskRequestView {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<Thing>,
    pub created_by: UserView,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub participants: Option<Vec<TaskRequestViewParticipant>>,
    pub request_txt: String,
    pub donors: Vec<TaskRequestDonorView>,
    pub reward_type: RewardType,
    pub currency: CurrencySymbol,
    pub wallet_id: Thing,
    pub due_at: DateTime<Utc>,
}

impl ViewFieldSelector for TaskRequestView {
    fn get_select_query_fields() -> String {
        "id,
        due_at,
        created_by.{id, username, full_name} as created_by,
        ->task_participant.{ user: out.{id, username, full_name},status} as participants,
        request_txt,
        ->task_donor.{id, user: out.{id, username, full_name}, amount: transaction.amount_out} as donors,
        reward_type,
        currency,
        wallet_id
        ".to_string()
    }
}

impl ViewRelateField for TaskRequestView {
    fn get_fields() -> &'static str {
        "id,
        due_at,
        created_by:created_by.{id, username, full_name},
        participants:->task_participant.{ user: out.{id, username, full_name},status},
        request_txt,
        donors:->task_donor.{id, user: out.{id, username, full_name}, amount: transaction.amount_out},
        reward_type,
        currency,
        wallet_id"
    }
}

#[derive(Debug, Deserialize)]
struct GetTaskByToUserQuery {
    status: Option<TaskParticipantStatus>,
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
    .get_by_creator::<TaskRequestView>(from_user, None)
    .await;

    println!(">>>.{:?}", list);
    let list = list?;
    serde_json::to_string(&list).map_err(|e| ctx.to_ctx_error(e.into()))
}

async fn reject_task_request(
    State(state): State<Arc<CtxState>>,
    ctx: Ctx,
    Path(task_id): Path<String>,
) -> CtxResult<Html<String>> {
    let task_service = TaskService::new(
        &state.db.client,
        &ctx,
        &state.event_sender,
        &state.db.user_notifications,
        &state.db.task_donors,
        &state.db.task_participants,
        &state.db.task_relates,
    );

    task_service.reject(&ctx.user_id()?, &task_id).await?;

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
    let task_service = TaskService::new(
        &state.db.client,
        &ctx,
        &state.event_sender,
        &state.db.user_notifications,
        &state.db.task_donors,
        &state.db.task_participants,
        &state.db.task_relates,
    );

    task_service.accept(&ctx.user_id()?, &task_id).await?;

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
    Json(input): Json<DeliverTaskRequestInput>,
) -> CtxResult<Html<String>> {
    let task_service = TaskService::new(
        &state.db.client,
        &ctx,
        &state.event_sender,
        &state.db.user_notifications,
        &state.db.task_donors,
        &state.db.task_participants,
        &state.db.task_relates,
    );

    task_service
        .deliver(
            &ctx.user_id()?,
            &task_id,
            TaskDeliveryData {
                post_id: input.post_id,
            },
        )
        .await?;

    ctx.to_htmx_or_json(CreatedResponse {
        id: task_id,
        uri: None,
        success: true,
    })
}

async fn upsert_donor(
    State(state): State<Arc<CtxState>>,
    ctx: Ctx,
    Path(task_id): Path<String>,
    JsonOrFormValidated(data): JsonOrFormValidated<TaskRequestOfferInput>,
) -> CtxResult<Html<String>> {
    let task_service = TaskService::new(
        &state.db.client,
        &ctx,
        &state.event_sender,
        &state.db.user_notifications,
        &state.db.task_donors,
        &state.db.task_participants,
        &state.db.task_relates,
    );

    let id = task_service
        .upsert_donor(
            &task_id,
            &ctx.user_id()?,
            TaskDonorData {
                amount: data.amount,
            },
        )
        .await?;

    ctx.to_htmx_or_json(CreatedResponse {
        success: true,
        id,
        uri: None,
    })
}
