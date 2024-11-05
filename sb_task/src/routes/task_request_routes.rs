use crate::entity::task_request_entitiy::{TaskRequest, TaskRequestDbService, TaskStatus};
use axum::extract::{Path, State};
use axum::response::Html;
use axum::routing::{get, post};
use axum::{Json, Router};
use sb_middleware::ctx::Ctx;
use sb_middleware::error::{AppError, CtxResult};
use sb_middleware::mw_ctx::CtxState;
use sb_middleware::utils::extractor_utils::JsonOrFormValidated;
use sb_middleware::utils::request_utils::CreatedResponse;
use sb_user_auth::entity::local_user_entity::LocalUserDbService;
use serde::{Deserialize, Serialize};
use surrealdb::sql::Thing;
use validator::Validate;
use sb_middleware::utils::string_utils::get_string_thing;

pub fn routes(state: CtxState) -> Router {
    Router::new()
        .route("/api/task_request", post(create_entity))
        .route("/api/task_request/received/post/:post_id", get(post_requests_received))
        .route("/api/task_request/given/post/:post_id", get(post_requests_given))
        .route("/api/task_request/:task_id/status", post(accept_task_request))
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

#[derive(Deserialize, Serialize)]
pub struct AcceptTaskRequestInput {
    pub accept: bool,
}

async fn post_requests_received(
    State(CtxState { _db, .. }): State<CtxState>,
    ctx: Ctx,
    Path(post_id): Path<String>,
) -> CtxResult<String> {
    let to_user = LocalUserDbService { db: &_db, ctx: &ctx }.get_ctx_user_thing().await?;
    let post_id = Thing::try_from(post_id).map_err(|e| ctx.to_ctx_error(AppError::Generic { description: "error into post_id Thing".to_string() }))?;

    let list = TaskRequestDbService { db: &_db, ctx: &ctx }.to_user_post_list(to_user, post_id).await?;
    serde_json::to_string(&list).map_err(|e| ctx.to_ctx_error(e.into()))
}

async fn post_requests_given(
    State(CtxState { _db, .. }): State<CtxState>,
    ctx: Ctx,
    Path(post_id): Path<String>,
) -> CtxResult<String> {
    let from_user = LocalUserDbService { db: &_db, ctx: &ctx }.get_ctx_user_thing().await?;
    let post_id = Thing::try_from(post_id).map_err(|e| ctx.to_ctx_error(AppError::Generic { description: "error into post_id Thing".to_string() }))?;

    let list = TaskRequestDbService { db: &_db, ctx: &ctx }.from_user_post_list(from_user, post_id).await?;
    serde_json::to_string(&list).map_err(|e| ctx.to_ctx_error(e.into()))
}

async fn user_task_requests_accepted(
    State(CtxState { _db, .. }): State<CtxState>,
    ctx: Ctx,
) -> CtxResult<String> {
    let from_user = LocalUserDbService { db: &_db, ctx: &ctx }.get_ctx_user_thing().await?;

    let list = TaskRequestDbService { db: &_db, ctx: &ctx }.to_user_status_list(from_user, TaskStatus::Accepted).await?;
    serde_json::to_string(&list).map_err(|e| ctx.to_ctx_error(e.into()))
}

async fn create_entity(State(CtxState { _db, .. }): State<CtxState>,
                       ctx: Ctx,
                       JsonOrFormValidated(t_request_input): JsonOrFormValidated<TaskRequestInput>,
) -> CtxResult<Html<String>> {
    let from_user = LocalUserDbService { db: &_db, ctx: &ctx }.get_ctx_user_thing().await?;

    let to_user = if t_request_input.to_user.len() > 0 {
        Thing::try_from(t_request_input.to_user).map_err(|e| AppError::Generic { description: "to_user error into id Thing".to_string() })?
    } else {
        return Err(ctx.to_ctx_error(AppError::Generic { description: "to_user must have value".to_string() }));
    };

    let content = if t_request_input.content.len() > 0 {
        t_request_input.content
    } else {
        return Err(ctx.to_ctx_error(AppError::Generic { description: "content must have value".to_string() }));
    };

    let offer_amount = if t_request_input.offer_amount > 0 {
        t_request_input.offer_amount
    } else {
        return Err(ctx.to_ctx_error(AppError::Generic { description: "offer_amount must have value greater than 0".to_string() }));
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


async fn accept_task_request(State(CtxState { _db, .. }): State<CtxState>,
                             ctx: Ctx,
                             Path(task_id): Path<String>,
                             Json(t_request_input): Json<AcceptTaskRequestInput>,
) -> CtxResult<Html<String>> {
    let to_user = LocalUserDbService { db: &_db, ctx: &ctx }.get_ctx_user_thing().await?;
    let task_id = get_string_thing(task_id)?;
    let status = match t_request_input.accept {
        true => TaskStatus::Accepted,
        false => TaskStatus::Rejected
    };
    TaskRequestDbService { db: &_db, ctx: &ctx }.update_status(to_user, task_id.clone(), status).await?;

    ctx.to_htmx_or_json_res(CreatedResponse { id: task_id.to_raw(), uri: None, success: true })
}
