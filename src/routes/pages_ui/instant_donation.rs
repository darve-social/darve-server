use crate::entities::task_donor::TaskDonor;
use crate::middleware::auth_with_login_access::AuthWithLoginAccess;
use crate::entities::task_request_user::{TaskParticipant, TaskParticipantStatus};
use crate::entities::user_auth::local_user_entity;
use crate::interfaces::repositories::task_request_ifce::TaskRequestRepositoryInterface;
use crate::middleware;
use crate::middleware::utils::db_utils::{Pagination, QryOrder};
use crate::models::view::task::{TaskRequestView, TaskViewForParticipant};
use crate::services::notification_service::NotificationService;
use crate::services::task_service::{TaskDonorData, TaskService};
use crate::utils::file::convert::convert_field_file_data;
use axum::extract::{DefaultBodyLimit, Path, Query, State};
use axum::routing::{get, post};
use axum::{Json, Router};
use axum_typed_multipart::{FieldData, TryFromMultipart, TypedMultipart};
use local_user_entity::LocalUserDbService;
use middleware::error::CtxResult;
use middleware::mw_ctx::CtxState;
use middleware::utils::extractor_utils::JsonOrFormValidated;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use axum::http::StatusCode;
use axum::response::{Html, IntoResponse, Redirect};
use tempfile::NamedTempFile;
use validator::Validate;

pub fn routes() -> Router<Arc<CtxState>> {
    Router::new()
        .route("/instant-donation/{task_id}", get(instant_donation_display))
}

async fn instant_donation_display(
    auth: Result<AuthWithLoginAccess, StatusCode>,
    State(state): State<Arc<CtxState>>,
    Path(task_id): Path<String>,
) -> Result<impl IntoResponse, Redirect> {

    let auth_data = auth.map_err(|_| Redirect::to("/login/twitch"))?;


    let task_service = TaskService::new(
        &state.db.client,
        &auth_data.ctx,
        &state.db.task_request,
        &state.db.task_donors,
        &state.db.task_participants,
        &state.db.access,
        &state.db.tags,
        NotificationService::new(
            &state.db.client,
            &auth_data.ctx,
            &state.event_sender,
            &state.db.user_notifications,
        ),
        state.file_storage.clone(),
    );

    Ok(Html::from("instant donation page".to_string()))
}