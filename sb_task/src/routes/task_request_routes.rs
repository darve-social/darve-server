use axum::extract::State;
use axum::http::{StatusCode};
use axum::response::{Response, IntoResponse};
use axum::routing::post;
use axum::Router;
use sb_middleware::ctx::Ctx;
use sb_middleware::error::CtxResult;
use sb_middleware::mw_ctx::CtxState;
use sb_middleware::utils::extractor_utils::JsonOrFormValidated;
use serde::{Deserialize, Serialize};
use validator::Validate;
use sb_middleware::error::AppError::RegisterFail;

pub fn routes(state: CtxState) -> Router {
    Router::new()
        .route("/api/task_request", post(create_entity))
        .with_state(state)
}

#[derive(Deserialize, Serialize, Validate)]
pub struct TaskRequestInput {

}

async fn create_entity(State(CtxState { _db, .. }): State<CtxState>,
                       ctx: Ctx,
                       JsonOrFormValidated(t_request_input): JsonOrFormValidated<TaskRequestInput>,
) -> CtxResult<Response> {
    Ok((StatusCode::OK, "oo").into_response())

}
