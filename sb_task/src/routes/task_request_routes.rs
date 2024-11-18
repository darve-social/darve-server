use crate::entity::task_request_entitiy::{TaskRequest, TaskRequestDbService, TaskStatus, UserTaskRole, TABLE_NAME};
use axum::body::Body;
use axum::extract::{DefaultBodyLimit, Path, Request, State};
use axum::http::uri::PathAndQuery;
use axum::http::{Response, StatusCode, Uri};
use axum::response::{Html, IntoResponse};
use axum::routing::{get, post};
use axum::{Json, Router};
use axum_typed_multipart::{FieldData, TryFromMultipart, TypedMultipart};
use sb_middleware::ctx::Ctx;
use sb_middleware::error::AppError::AuthorizationFail;
use sb_middleware::error::{AppError, CtxError, CtxResult};
use sb_middleware::mw_ctx::CtxState;
use sb_middleware::utils::db_utils::IdentIdName;
use sb_middleware::utils::extractor_utils::JsonOrFormValidated;
use sb_middleware::utils::request_utils::CreatedResponse;
use sb_middleware::utils::string_utils::get_string_thing;
use sb_user_auth::entity::local_user_entity::LocalUserDbService;
use serde::{Deserialize, Serialize};
use serde_json::from_str;
use std::fmt::{Display, Formatter};
use std::path::Path as FPath;
use surrealdb::sql::{Id, Thing};
use tempfile::NamedTempFile;
use tower::util::ServiceExt;
use tower_http::services::fs::ServeFileSystemResponseBody;
use validator::Validate;
use crate::entity::task_request_offer_entity::{TaskRequestOffer, TaskRequestOfferDbService};

pub const DELIVERIES_URL_BASE: &str = "/tasks/*file";

pub fn routes(state: CtxState) -> Router {
    Router::new()
        .route("/api/task_request", post(create_entity))
        .route("/api/task_request/received/post/:post_id", get(post_requests_received))
        .route("/api/task_request/given/post/:post_id", get(post_requests_given))
        .route("/api/task_request/:task_id/accept", post(accept_task_request))
        .route("/api/task_request/:task_id/deliver", post(deliver_task_request))
        .route("/api/task_request/:task_id/offer", post(add_task_request_offer))
        .route(DELIVERIES_URL_BASE, get(serve_task_deliverable_file))
        .layer(DefaultBodyLimit::max(1024 * 1024 * 30))
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
    pub offer_amount: Option<i64>,
}

#[derive(Deserialize, Serialize, Validate)]
pub struct TaskRequestOfferInput {
    pub amount: i64,
}

#[derive(Deserialize, Serialize)]
pub struct AcceptTaskRequestInput {
    pub accept: bool,
}

#[derive(Validate, TryFromMultipart)]
pub struct DeliverTaskRequestInput {
    #[form_data(limit = "30MiB")]
    pub file_1: FieldData<NamedTempFile>,
}

async fn post_requests_received(
    State(CtxState { _db, .. }): State<CtxState>,
    ctx: Ctx,
    Path(post_id): Path<String>,
) -> CtxResult<String> {
    let to_user = LocalUserDbService { db: &_db, ctx: &ctx }.get_ctx_user_thing().await?;
    let post_id = Thing::try_from(post_id).map_err(|e| ctx.to_ctx_error(AppError::Generic { description: "error into post_id Thing".to_string() }))?;

    let list = TaskRequestDbService { db: &_db, ctx: &ctx }.user_post_list(UserTaskRole::ToUser, to_user, post_id).await?;
    serde_json::to_string(&list).map_err(|e| ctx.to_ctx_error(e.into()))
}

async fn post_requests_given(
    State(CtxState { _db, .. }): State<CtxState>,
    ctx: Ctx,
    Path(post_id): Path<String>,
) -> CtxResult<String> {
    let from_user = LocalUserDbService { db: &_db, ctx: &ctx }.get_ctx_user_thing().await?;
    let post_id = Thing::try_from(post_id).map_err(|e| ctx.to_ctx_error(AppError::Generic { description: "error into post_id Thing".to_string() }))?;

    let list = TaskRequestDbService { db: &_db, ctx: &ctx }.user_post_list(UserTaskRole::FromUser, from_user, post_id).await?;
    serde_json::to_string(&list).map_err(|e| ctx.to_ctx_error(e.into()))
}

async fn user_task_requests_accepted(
    State(CtxState { _db, .. }): State<CtxState>,
    ctx: Ctx,
) -> CtxResult<String> {
    let from_user = LocalUserDbService { db: &_db, ctx: &ctx }.get_ctx_user_thing().await?;

    let list = TaskRequestDbService { db: &_db, ctx: &ctx }.user_status_list(UserTaskRole::FromUser, from_user, TaskStatus::Accepted).await?;
    serde_json::to_string(&list).map_err(|e| ctx.to_ctx_error(e.into()))
}

async fn user_task_requests_delivered(
    State(CtxState { _db, .. }): State<CtxState>,
    ctx: Ctx,
) -> CtxResult<String> {
    let from_user = LocalUserDbService { db: &_db, ctx: &ctx }.get_ctx_user_thing().await?;

    let list = TaskRequestDbService { db: &_db, ctx: &ctx }.user_status_list(UserTaskRole::FromUser, from_user, TaskStatus::Delivered).await?;
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

    let offer_amount = t_request_input.offer_amount.unwrap_or(0);


    let post_id = t_request_input.post_id.unwrap_or("".to_string());
    let post = if post_id.len() > 0 {
        Some(Thing::try_from(post_id).map_err(|e| AppError::Generic { description: "post_id error into id Thing".to_string() })?)
    } else {
        None
    };

    let t_req_id = Thing::from((TABLE_NAME, Id::ulid()));
    let offer = TaskRequestOfferDbService { db: &_db, ctx: &ctx }.create_update(TaskRequestOffer {
        id: None,
        task_request: t_req_id.clone(),
        user: from_user.clone(),
        amount: offer_amount,
        r_created: None,
        r_updated: None,
    }).await?;
    let t_request = TaskRequestDbService { db: &_db, ctx: &ctx }.create(TaskRequest { id: Some(t_req_id), from_user, to_user, request_post: post, request_txt: content, offers: vec![offer.id.unwrap()], status: TaskStatus::Requested.to_string(), deliverables: None, deliverables_post: None, r_created: None, r_updated: None }).await?;

    ctx.to_htmx_or_json_res(CreatedResponse { id: t_request.id.unwrap().to_raw(), uri: None, success: true })
}

async fn serve_task_deliverable_file(State(CtxState { _db, uploads_serve_dir, .. }): State<CtxState>,
                                     ctx: Ctx,
                                     Path(path): Path<String>,
) -> Result<Response<ServeFileSystemResponseBody>, CtxError> {
    let user = get_string_thing(ctx.user_id()?)?;

    let task_file = TaskDeliverableFileName::try_from(path.clone())?;
    let task = TaskRequestDbService { db: &_db, ctx: &ctx }.get(IdentIdName::Id(task_file.task_id.to_raw())).await?;
    if task.from_user != user {
        return Err(ctx.to_ctx_error(AuthorizationFail { required: "Not authorised".to_string() }));
    }

    let uri = Uri::from(PathAndQuery::try_from(path).unwrap());
    let req = Request::builder().uri(uri).body(Body::empty()).unwrap();
    let res = uploads_serve_dir.oneshot(req).await;
    res.map_err(|e| ctx.to_ctx_error(AppError::Generic { description: "Error getting file".to_string() }))
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
    TaskRequestDbService { db: &_db, ctx: &ctx }.update_status_received_by_user(to_user, task_id.clone(), status, None).await?;

    ctx.to_htmx_or_json_res(CreatedResponse { id: task_id.to_raw(), uri: None, success: true })
}

async fn deliver_task_request(State(CtxState { _db, uploads_dir, .. }): State<CtxState>,
                              ctx: Ctx,
                              Path(task_id): Path<String>,
                              TypedMultipart(t_request_input): TypedMultipart<DeliverTaskRequestInput>,
) -> CtxResult<Html<String>> {
    let to_user = LocalUserDbService { db: &_db, ctx: &ctx }.get_ctx_user_thing().await?;
    let task_id = get_string_thing(task_id)?;

    let file_name = t_request_input.file_1.metadata.file_name.unwrap();
    let ext = file_name.split(".").last().ok_or(AppError::Generic { description: "File has no extension".to_string() })?;

    let file_name: String = TaskDeliverableFileName { task_id: task_id.clone(), file_nr: 1, ext: ext.to_string() }.to_string();
    let path = FPath::new(&uploads_dir).join(file_name.clone());
    t_request_input.file_1.contents.persist(path.clone())
        .map_err(|e| ctx.to_ctx_error(AppError::Generic { description: "Upload failed".to_string() }))?;
    let file_uri = format!("{DELIVERIES_URL_BASE}/{file_name}");

    TaskRequestDbService { db: &_db, ctx: &ctx }.update_status_received_by_user(to_user, task_id.clone(), TaskStatus::Delivered, Some(vec![file_uri])).await?;

    ctx.to_htmx_or_json_res(CreatedResponse { id: task_id.to_raw(), uri: None, success: true })
}

async fn add_task_request_offer(State(CtxState { _db, .. }): State<CtxState>,
                                ctx: Ctx,
                                Path(task_id): Path<String>,
                                JsonOrFormValidated(t_request_offer_input): JsonOrFormValidated<TaskRequestOfferInput>,
) -> CtxResult<Html<String>> {
    let from_user = LocalUserDbService { db: &_db, ctx: &ctx }.get_ctx_user_thing().await?;
    let task_offer = TaskRequestOfferDbService{ db: &_db, ctx: &ctx }.add_to_task_offers(get_string_thing(task_id)?, from_user, t_request_offer_input.amount).await?;
    ctx.to_htmx_or_json_res(CreatedResponse{
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
        f.write_str(format!("tid_{}-file_{}.{}", self.task_id.id.to_raw(), self.file_nr, self.ext).as_str())
    }
}

impl TryFrom<String> for TaskDeliverableFileName {
    type Error = AppError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        let error = AppError::Generic { description: "Can not parse task file".to_string() };
        let tid_fname = value.split_once("-").ok_or(error.clone())?;
        let tid = tid_fname.0.split_once("_").ok_or(error.clone())?.1;
        let task_id = Thing::from((crate::entity::task_request_entitiy::TABLE_NAME, tid));

        let fnr_ext = tid_fname.1.split_once(".").ok_or(error.clone())?;
        let fnr = fnr_ext.0.split_once("_").ok_or(error.clone())?;
        let file_nr: i8 = from_str(fnr.1).map_err(|e| error.clone())?;
        let ext = fnr_ext.1.to_string();

        Ok(TaskDeliverableFileName { ext, file_nr, task_id })
    }
}
