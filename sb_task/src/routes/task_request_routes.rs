use crate::entity::task_request_entitiy::{TaskRequest, TaskRequestDbService, TaskStatus};
use axum::extract::State;
use axum::response::Html;
use axum::routing::{get, post};
use axum::Router;
use sb_middleware::ctx::Ctx;
use sb_middleware::error::{AppError, CtxResult};
use sb_middleware::mw_ctx::CtxState;
use sb_middleware::utils::extractor_utils::JsonOrFormValidated;
use sb_middleware::utils::request_utils::CreatedResponse;
use sb_user_auth::entity::local_user_entity::LocalUserDbService;
use serde::{Deserialize, Serialize};
use surrealdb::sql::Thing;
use validator::Validate;

pub fn routes(state: CtxState) -> Router {
    Router::new()
        .route("/api/task_request", post(create_entity))
        .route("/api/task_requests_received", get(get_user_task_requests_received))
        .route("/api/task_requests_given", get(get_user_task_requests_given))
        .with_state(state)
}

#[derive(Deserialize, Serialize, Validate)]
pub struct TaskRequestInput {
    #[validate(length(min = 5, message = "Min 5 characters"))]
    pub content: String,
    #[validate(length(min = 5, message = "Min 5 characters"))]
    pub to_user: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub post_id: Option<String>,
    pub offer_amount: u64,
}

async fn get_user_task_requests_received(
    State(CtxState { _db, .. }): State<CtxState>,
    ctx: Ctx,
) -> CtxResult<String> {
    let to_user = LocalUserDbService { db: &_db, ctx: &ctx }.get_ctx_user_thing().await?;
    let list = TaskRequestDbService { db: &_db, ctx: &ctx }.get_to_user_task_list(to_user).await?;
    serde_json::to_string(&list).map_err(|e| ctx.to_api_error(e.into()))
}

async fn get_user_task_requests_given(
    State(CtxState { _db, .. }): State<CtxState>,
    ctx: Ctx,
) -> CtxResult<String> {
    let from_user = LocalUserDbService { db: &_db, ctx: &ctx }.get_ctx_user_thing().await?;
    let list = TaskRequestDbService { db: &_db, ctx: &ctx }.get_from_user_task_list(from_user).await?;
    serde_json::to_string(&list).map_err(|e| ctx.to_api_error(e.into()))
}

async fn create_entity(State(CtxState { _db, .. }): State<CtxState>,
                       ctx: Ctx,
                       JsonOrFormValidated(t_request_input): JsonOrFormValidated<TaskRequestInput>,
) -> CtxResult<Html<String>> {
    let from_user = LocalUserDbService { db: &_db, ctx: &ctx }.get_ctx_user_thing().await?;

    let to_user = if t_request_input.to_user.len() > 0 {
        Thing::try_from(t_request_input.to_user).map_err(|e| AppError::Generic { description: "to_user error into id Thing".to_string() })?
    } else {
        return Err(ctx.to_api_error(AppError::Generic { description: "to_user must have value".to_string() }));
    };

    let content = if t_request_input.content.len() > 0 {
        t_request_input.content
    } else {
        return Err(ctx.to_api_error(AppError::Generic { description: "content must have value".to_string() }));
    };

    let offer_amount = if t_request_input.offer_amount > 0 {
        t_request_input.offer_amount
    } else {
        return Err(ctx.to_api_error(AppError::Generic { description: "offer_amount must have value greater than 0".to_string() }));
    };

    let post_id = t_request_input.post_id.unwrap_or("".to_string());
    let post = if post_id.len() > 0 {
        Some(Thing::try_from(post_id).map_err(|e| AppError::Generic { description: "post_id error into id Thing".to_string() })?)
    } else {
        None
    };

    let t_request = TaskRequestDbService { db: &_db, ctx: &ctx }.create(TaskRequest { id: None, from_user, to_user, post, content, offer_amount, status: TaskStatus::Requested.to_string(), r_created: None, r_updated: None }).await?;

    ctx.to_htmx_or_json_res(CreatedResponse { id: t_request.id.unwrap().to_raw(), uri: None, success: true })
}
