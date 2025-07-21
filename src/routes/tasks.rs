use crate::entities::task::task_request_entity::{self};
use crate::entities::task_request_user::TaskParticipantStatus;
use crate::entities::user_auth::local_user_entity;
use crate::entities::wallet::wallet_entity;
use crate::middleware;
use crate::middleware::utils::db_utils::ViewRelateField;
use crate::models::web::{TaskRequestDonorView, UserView};
use crate::services::task_service::{TaskDeliveryData, TaskDonorData, TaskService};
use axum::extract::{Path, Query, State};
use axum::routing::{get, post};
use axum::{Json, Router};
use chrono::{DateTime, Utc};
use local_user_entity::LocalUserDbService;
use middleware::ctx::Ctx;
use middleware::error::CtxResult;
use middleware::mw_ctx::CtxState;
use middleware::utils::db_utils::ViewFieldSelector;
use middleware::utils::extractor_utils::JsonOrFormValidated;
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
    #[validate(range(min = 100))]
    pub amount: u64,
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
) -> CtxResult<Json<Vec<TaskRequestView>>> {
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
    Ok(Json(list))
}

async fn user_requests_given(
    State(state): State<Arc<CtxState>>,
    ctx: Ctx,
) -> CtxResult<Json<Vec<TaskRequestView>>> {
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
    .await?;

    Ok(Json(list))
}

async fn reject_task_request(
    State(state): State<Arc<CtxState>>,
    ctx: Ctx,
    Path(task_id): Path<String>,
) -> CtxResult<()> {
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

    Ok(())
}

async fn accept_task_request(
    State(state): State<Arc<CtxState>>,
    ctx: Ctx,
    Path(task_id): Path<String>,
) -> CtxResult<()> {
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

    Ok(())
}

async fn deliver_task_request(
    State(state): State<Arc<CtxState>>,
    ctx: Ctx,
    Path(task_id): Path<String>,
    Json(input): Json<DeliverTaskRequestInput>,
) -> CtxResult<()> {
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

    Ok(())
}

async fn upsert_donor(
    State(state): State<Arc<CtxState>>,
    ctx: Ctx,
    Path(task_id): Path<String>,
    JsonOrFormValidated(data): JsonOrFormValidated<TaskRequestOfferInput>,
) -> CtxResult<()> {
    let task_service = TaskService::new(
        &state.db.client,
        &ctx,
        &state.event_sender,
        &state.db.user_notifications,
        &state.db.task_donors,
        &state.db.task_participants,
        &state.db.task_relates,
    );

    let _ = task_service
        .upsert_donor(
            &task_id,
            &ctx.user_id()?,
            TaskDonorData {
                amount: data.amount,
            },
        )
        .await?;

    Ok(())
}
