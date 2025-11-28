use axum::{extract::State, routing::get, Json, Router};
use std::sync::Arc;

use crate::{
    entities::user_auth::local_user_entity::LocalUserDbService,
    middleware::{auth_with_login_access::AuthWithLoginAccess, error::CtxResult, mw_ctx::CtxState},
    models::view::task::TaskRequestView,
};

pub fn routes() -> Router<Arc<CtxState>> {
    Router::new().route("/api/admin/tasks", get(get_tasks))
}

async fn get_tasks(
    auth_data: AuthWithLoginAccess,
    State(state): State<Arc<CtxState>>,
) -> CtxResult<Json<Vec<TaskRequestView>>> {
    let user_id = auth_data.user_thing_id();
    let user_repository = LocalUserDbService {
        db: &state.db.client,
        ctx: &auth_data.ctx,
    };

    let _ = user_repository.get_by_id(&user_id).await?;

    let super_tasks = state
        .darve_tasks
        .create_public(&user_id, &state.event_sender)
        .await?;
    let weekly_tasks = state
        .darve_tasks
        .create_private(&user_id, &state.event_sender)
        .await?;

    let tasks = super_tasks
        .into_iter()
        .chain(weekly_tasks)
        .collect::<Vec<TaskRequestView>>();
    Ok(Json(tasks))
}
