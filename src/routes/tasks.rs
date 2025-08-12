use crate::entities::task::task_request_entity::{self};
use crate::entities::task_request_user::TaskParticipantStatus;
use crate::entities::user_auth::local_user_entity;
use crate::middleware;
use crate::middleware::auth_with_login_access::AuthWithLoginAccess;
use crate::middleware::utils::db_utils::{Pagination, QryOrder};
use crate::models::view::task::{TaskRequestView, TaskViewForParticipant};
use crate::services::task_service::{TaskDeliveryData, TaskDonorData, TaskService};
use axum::extract::{Path, Query, State};
use axum::routing::{get, post};
use axum::{Json, Router};
use local_user_entity::LocalUserDbService;
use middleware::ctx::Ctx;
use middleware::error::CtxResult;
use middleware::mw_ctx::CtxState;
use middleware::utils::extractor_utils::JsonOrFormValidated;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use task_request_entity::TaskRequestDbService;
use validator::Validate;

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

#[derive(Debug, Deserialize)]
struct GetTaskByToUserQuery {
    status: Option<TaskParticipantStatus>,
    is_ended: Option<bool>,
    order_dir: Option<QryOrder>,
    start: Option<u32>,
    count: Option<u16>,
}
async fn user_requests_received(
    State(state): State<Arc<CtxState>>,
    auth_data: AuthWithLoginAccess,
    Query(query): Query<GetTaskByToUserQuery>,
) -> CtxResult<Json<Vec<TaskViewForParticipant>>> {
    let user_id = LocalUserDbService {
        db: &state.db.client,
        ctx: &auth_data.ctx,
    }
    .get_ctx_user_thing()
    .await?;

    let task_service = TaskRequestDbService {
        db: &state.db.client,
        ctx: &auth_data.ctx,
    };

    let pagination = Pagination {
        order_by: None,
        order_dir: query.order_dir,
        count: query.count.unwrap_or(20),
        start: query.start.unwrap_or(0),
    };

    let list = task_service
        .get_by_user::<TaskRequestView>(&user_id, query.status, query.is_ended, pagination)
        .await?
        .into_iter()
        .map(|view| TaskViewForParticipant::from_view(view, &user_id))
        .collect::<Vec<TaskViewForParticipant>>();

    Ok(Json(list))
}

async fn user_requests_given(
    State(state): State<Arc<CtxState>>,
    auth_data: AuthWithLoginAccess,
) -> CtxResult<Json<Vec<TaskRequestView>>> {
    let from_user = LocalUserDbService {
        db: &state.db.client,
        ctx: &auth_data.ctx,
    }
    .get_ctx_user_thing()
    .await?;

    let list = TaskRequestDbService {
        db: &state.db.client,
        ctx: &auth_data.ctx,
    }
    .get_by_creator::<TaskRequestView>(from_user, None)
    .await?;
    Ok(Json(list))
}

async fn reject_task_request(
    State(state): State<Arc<CtxState>>,
    auth_data: AuthWithLoginAccess,
    Path(task_id): Path<String>,
) -> CtxResult<()> {
    let task_service = TaskService::new(
        &state.db.client,
        &auth_data.ctx,
        &state.event_sender,
        &state.db.user_notifications,
        &state.db.task_donors,
        &state.db.task_participants,
        &state.db.task_relates,
    );

    task_service.reject(&auth_data.user_id, &task_id).await?;

    Ok(())
}

async fn accept_task_request(
    State(state): State<Arc<CtxState>>,
    auth_data: AuthWithLoginAccess,
    Path(task_id): Path<String>,
) -> CtxResult<()> {
    let task_service = TaskService::new(
        &state.db.client,
        &auth_data.ctx,
        &state.event_sender,
        &state.db.user_notifications,
        &state.db.task_donors,
        &state.db.task_participants,
        &state.db.task_relates,
    );

    task_service.accept(&auth_data.user_id, &task_id).await?;

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
