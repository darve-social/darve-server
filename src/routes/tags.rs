use std::sync::Arc;

use axum::{
    extract::{Query, State},
    routing::get,
    Json, Router,
};
use serde::Deserialize;

use crate::{
    interfaces::repositories::tags::TagsRepositoryInterface,
    middleware::{
        error::CtxResult,
        mw_ctx::CtxState,
        utils::db_utils::{Pagination, QryOrder},
    },
};

pub fn routes() -> Router<Arc<CtxState>> {
    Router::new().route("/api/tags", get(get_tags))
}

#[derive(Debug, Deserialize)]
struct TagGetQuery {
    pub start_with: Option<String>,
    pub order_dir: Option<QryOrder>,
    pub start: Option<u32>,
    pub count: Option<u16>,
}

async fn get_tags(
    State(state): State<Arc<CtxState>>,
    Query(query): Query<TagGetQuery>,
) -> CtxResult<Json<Vec<String>>> {
    let tags = state
        .db
        .tags
        .get(
            query.start_with,
            Pagination {
                order_by: None,
                order_dir: query.order_dir,
                count: query.count.unwrap_or(20),
                start: query.start.unwrap_or(0),
            },
        )
        .await?;
    Ok(Json(tags))
}
