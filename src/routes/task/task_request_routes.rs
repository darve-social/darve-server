use crate::entities::task::task_request_entity::{self};
use crate::entities::task_request_user::TaskParticipantStatus;
use crate::entities::user_auth::local_user_entity;
use crate::entities::wallet::wallet_entity;
use crate::middleware;
use crate::models::web::{TaskRequestDonorView, UserView};
use crate::services::task_service::{
    TaskDeliveryData, TaskDonorData, TaskRequestInput, TaskService,
};
use axum::extract::{Path, Query, State};
use axum::response::Html;
use axum::routing::{get, post};
use axum::Router;
use axum_typed_multipart::{TryFromMultipart, TypedMultipart};
use local_user_entity::LocalUserDbService;
use middleware::ctx::Ctx;
use middleware::error::CtxResult;
use middleware::mw_ctx::CtxState;
use middleware::utils::db_utils::{IdentIdName, ViewFieldSelector};
use middleware::utils::extractor_utils::JsonOrFormValidated;
use middleware::utils::request_utils::CreatedResponse;
use middleware::utils::string_utils::get_string_thing;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use surrealdb::sql::Thing;
use task_request_entity::{RewardType, TaskRequestDbService};
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
pub struct TaskRequestOfferInput {
    pub amount: u64,
    pub currency: Option<CurrencySymbol>,
}

#[derive(Validate, TryFromMultipart)]
pub struct DeliverTaskRequestInput {
    // currently only use post_id
    // #[form_data(limit = "30MiB")]
    // pub file_1: Option<FieldData<NamedTempFile>>,
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
    pub creator: UserView,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub participants: Option<Vec<TaskRequestViewParticipant>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub request_post: Option<Thing>,
    pub request_txt: String,
    pub donors: Vec<TaskRequestDonorView>,
    pub reward_type: RewardType,
    pub currency: CurrencySymbol,
    pub wallet_id: Thing,
    pub on_post: Option<Thing>,
}

impl ViewFieldSelector for TaskRequestView {
    fn get_select_query_fields(_ident: &IdentIdName) -> String {
        "id, 
        from_user.{id, username, full_name} as creator,
        on_post, 
        request_txt, 
        reward_type, 
        currency,
        wallet_id,
        ->task_participant.{
            user: out.{id, username, full_name},        
            status
        } as participants,
        ->task_donor.{id, user: out.{id, username, full_name}, amount: transaction.amount_out} as donors"
            .to_string()
    }
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
    .await;

    println!("task request list: {:?}", list);
    let list = list?;
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
    JsonOrFormValidated(data): JsonOrFormValidated<TaskRequestInput>,
) -> CtxResult<Html<String>> {
    let task_service = TaskService::new(
        &state.db.client,
        &ctx,
        &state.event_sender,
        &state.db.user_notifications,
        &state.db.task_donors,
        &state.db.task_participants,
    );

    let task = task_service.create(&ctx.user_id()?, data).await?;

    ctx.to_htmx_or_json(CreatedResponse {
        id: task.id.as_ref().unwrap().to_raw(),
        uri: None,
        success: true,
    })
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
    TypedMultipart(input): TypedMultipart<DeliverTaskRequestInput>,
) -> CtxResult<Html<String>> {
    let task_service = TaskService::new(
        &state.db.client,
        &ctx,
        &state.event_sender,
        &state.db.user_notifications,
        &state.db.task_donors,
        &state.db.task_participants,
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
