use std::sync::Arc;

use axum::{extract::State, routing::get, Json, Router};

use crate::{
    interfaces::repositories::tags::TagsRepositoryInterface,
    middleware::{error::CtxResult, mw_ctx::CtxState},
};

pub fn routes() -> Router<Arc<CtxState>> {
    Router::new().route("/api/tags", get(get_tags))
}

async fn get_tags(State(state): State<Arc<CtxState>>) -> CtxResult<Json<Vec<String>>> {
    let tags = state.db.tags.get().await?;
    Ok(Json(tags))
}
