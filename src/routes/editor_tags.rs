use std::sync::Arc;

use axum::{extract::State, routing::get, Json, Router};

use crate::{
    entities::tag::EditorTag,
    interfaces::repositories::editor_tags::EditorTagsRepositoryInterface,
    middleware::{error::CtxResult, mw_ctx::CtxState},
};

pub fn routes() -> Router<Arc<CtxState>> {
    Router::new().route("/api/editor_tags", get(get_tags))
}

async fn get_tags(State(state): State<Arc<CtxState>>) -> CtxResult<Json<Vec<EditorTag>>> {
    let tags = state.db.editor_tags.get().await?;
    Ok(Json(tags))
}
