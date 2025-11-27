use axum::{extract::State, routing::get, Json, Router};
use std::sync::Arc;

use crate::{
    entities::{
        community::discussion_entity::DiscussionDbService,
        task::task_request_entity::{TaskRequestDbService, TaskRequestType},
        user_auth::local_user_entity::{LocalUserDbService, UserRole},
    },
    middleware::{
        auth_with_login_access::AuthWithLoginAccess,
        error::{AppError, CtxResult},
        mw_ctx::CtxState,
    },
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

    let admins = user_repository.get_by_role(UserRole::Admin).await?;

    let admin_disc = admins
        .first()
        .map(|u| DiscussionDbService::get_profile_discussion_id(u.id.as_ref().unwrap()));

    if admin_disc.is_none() {
        return Err(AppError::Forbidden.into());
    }

    let admin_disc_id = admin_disc.as_ref().unwrap().id.to_raw();

    let task_repository = TaskRequestDbService {
        db: &state.db.client,
        ctx: &auth_data.ctx,
    };

    let weekly_tasks = task_repository
        .get_by_public_disc::<TaskRequestView>(
            &admin_disc_id,
            &user_id,
            Some(TaskRequestType::Private),
            None,
            Some(false),
        )
        .await?;
    let super_tasks = task_repository
        .get_by_public_disc::<TaskRequestView>(
            &admin_disc_id,
            &user_id,
            Some(TaskRequestType::Public),
            None,
            Some(false),
        )
        .await?;

    let tasks = super_tasks
        .into_iter()
        .chain(weekly_tasks)
        .collect::<Vec<TaskRequestView>>();
    Ok(Json(tasks))
}
