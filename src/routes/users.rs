use crate::{
    middleware::utils::db_utils::{Pagination, QryOrder},
    utils::validate_utils::validate_social_links,
};
use axum::{
    extract::{DefaultBodyLimit, Multipart, Query, State},
    response::{IntoResponse, Response},
    routing::{get, patch, post},
    Json, Router,
};
use axum_typed_multipart::TypedMultipart;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::sync::Arc;
use validator::Validate;

use crate::{
    entities::{
        community::{
            discussion_entity::DiscussionDbService, post_entity::Post,
            post_stream_entity::PostStreamDbService,
        },
        user_auth::{
            authentication_entity::AuthenticationDbService, local_user_entity::LocalUserDbService,
        },
    },
    interfaces::file_storage::FileStorageInterface,
    middleware::{
        error::{AppError, CtxResult},
        mw_ctx::CtxState,
        utils::{
            extractor_utils::{DiscussionParams, JsonOrFormValidated},
            string_utils::get_str_thing,
        },
    },
    routes::community::discussion_routes::DiscussionPostView,
    services::{
        post_service::{PostInput, PostService, PostView},
        user_service::UserService,
    },
    utils::{self, file::convert::FileUpload},
};

use utils::validate_utils::validate_birth_date;
use utils::validate_utils::validate_username;

pub fn routes(upload_max_size_mb: u64) -> Router<Arc<CtxState>> {
    let max_bytes_val = (1024 * 1024 * upload_max_size_mb) as usize;
    Router::new()
        .route("/api/users/current/password", patch(reset_password))
        .route("/api/users/current/password", post(set_password))
        .route(
            "/api/users/current/following/posts",
            get(get_following_posts),
        )
        .route(
            "/api/users/current/email/verification/start",
            post(email_verification_start),
        )
        .route(
            "/api/users/current/email/verification/confirm",
            post(email_verification_confirm),
        )
        .route("/api/users/current", patch(update_user))
        .route("/api/users/current", get(get_user))
        .route("/api/users/current/posts", post(create_post))
        .route("/api/users/current/posts", get(get_posts))
        .route("/api/users", get(search_users))
        .layer(DefaultBodyLimit::max(max_bytes_val))
}

#[derive(Debug, Deserialize, Validate)]
struct SetPasswordInput {
    #[validate(length(min = 6, message = "Min 6 characters"))]
    password: String,
}

async fn set_password(
    auth_data: AuthWithLoginAccess,
    State(state): State<Arc<CtxState>>,
    JsonOrFormValidated(data): JsonOrFormValidated<SetPasswordInput>,
) -> CtxResult<Response> {
    let user_service = UserService::new(
        LocalUserDbService {
            db: &state.db.client,
            ctx: &auth_data.ctx,
        },
        &state.email_sender,
        state.verification_code_ttl,
        AuthenticationDbService {
            db: &state.db.client,
            ctx: &auth_data.ctx,
        },
        &state.db.verification_code,
    );

    user_service
        .set_password(&auth_data.user_id, &data.password)
        .await?;

    Ok(().into_response())
}

#[derive(Debug, Deserialize, Validate)]
struct ResetPasswordInput {
    #[validate(length(min = 6, message = "Min 6 characters"))]
    old_password: String,
    #[validate(length(min = 6, message = "Min 6 characters"))]
    new_password: String,
}

async fn reset_password(
    auth_data: AuthWithLoginAccess,
    State(state): State<Arc<CtxState>>,
    JsonOrFormValidated(data): JsonOrFormValidated<ResetPasswordInput>,
) -> CtxResult<String> {
    let user_service = UserService::new(
        LocalUserDbService {
            db: &state.db.client,
            ctx: &auth_data.ctx,
        },
        &state.email_sender,
        state.verification_code_ttl,
        AuthenticationDbService {
            db: &state.db.client,
            ctx: &auth_data.ctx,
        },
        &state.db.verification_code,
    );

    let _ = user_service
        .update_password(&auth_data.user_id, &data.new_password, &data.old_password)
        .await?;

    Ok("Password updated successfully.".to_string())
}

#[derive(Debug, Deserialize, Validate)]
pub struct EmailVerificationStartInput {
    #[validate(email)]
    pub email: String,
}
async fn email_verification_start(
    auth_data: AuthWithLoginAccess,
    State(ctx_state): State<Arc<CtxState>>,
    JsonOrFormValidated(data): JsonOrFormValidated<EmailVerificationStartInput>,
) -> CtxResult<()> {
    let user_service = UserService::new(
        LocalUserDbService {
            db: &ctx_state.db.client,
            ctx: &auth_data.ctx,
        },
        &ctx_state.email_sender,
        ctx_state.verification_code_ttl,
        AuthenticationDbService {
            db: &ctx_state.db.client,
            ctx: &auth_data.ctx,
        },
        &ctx_state.db.verification_code,
    );

    user_service
        .start_email_verification(&auth_data.user_thing_id(), &data.email)
        .await?;

    Ok(())
}

#[derive(Debug, Deserialize, Validate)]

pub struct EmailVerificationConfirmInput {
    #[validate(email)]
    pub email: String,
    #[validate(length(equal = 6, message = "Code must be 6 characters long"))]
    pub code: String,
}

async fn email_verification_confirm(
    auth_data: AuthWithLoginAccess,
    State(ctx_state): State<Arc<CtxState>>,
    JsonOrFormValidated(data): JsonOrFormValidated<EmailVerificationConfirmInput>,
) -> CtxResult<()> {
    let user_service = UserService::new(
        LocalUserDbService {
            db: &ctx_state.db.client,
            ctx: &auth_data.ctx,
        },
        &ctx_state.email_sender,
        ctx_state.verification_code_ttl,
        AuthenticationDbService {
            db: &ctx_state.db.client,
            ctx: &auth_data.ctx,
        },
        &ctx_state.db.verification_code,
    );

    user_service
        .email_confirmation(&auth_data.user_thing_id(), &data.code, &data.email)
        .await?;

    Ok(())
}

async fn get_following_posts(
    State(state): State<Arc<CtxState>>,
    auth_data: AuthWithLoginAccess,
) -> CtxResult<Json<Vec<DiscussionPostView>>> {
    let local_user_db_service = LocalUserDbService {
        db: &state.db.client,
        ctx: &auth_data.ctx,
    };
    let user = local_user_db_service
        .get_by_id(&auth_data.user_thing_id())
        .await?;
    let data = PostStreamDbService {
        db: &state.db.client,
        ctx: &auth_data.ctx,
    }
    .get_posts::<DiscussionPostView>(user.id.as_ref().unwrap().clone())
    .await?;

    Ok(Json(data))
}

#[derive(Validate, Debug)]
pub struct ProfileSettingsFormInput {
    #[validate(custom(function = "validate_username"))]
    pub username: Option<String>,
    #[validate(length(min = 6, message = "Min 6 character"))]
    pub full_name: Option<String>,
    pub image_url: Option<FileUpload>,
    #[validate(custom(function = "validate_social_links"))]
    pub social_links: Option<Vec<String>>,
    #[validate(custom(function = "validate_birth_date", message = "Birth date is invalid"))]
    pub birth_date: Option<DateTime<Utc>>,
}

impl ProfileSettingsFormInput {
    async fn try_from_multipart(multipart: &mut Multipart) -> CtxResult<Self> {
        let mut form = ProfileSettingsFormInput {
            username: None,
            full_name: None,
            image_url: None,
            social_links: None,
            birth_date: None,
        };

        while let Some(field) = multipart.next_field().await.unwrap() {
            if let Some(name) = field.name() {
                match name {
                    "username" => {
                        form.username = Some(field.text().await.unwrap_or_default());
                    }
                    "full_name" => {
                        form.full_name = Some(field.text().await.unwrap_or_default());
                    }
                    "birth_date" => {
                        let value: String = field.text().await.unwrap_or_default();
                        let date = DateTime::parse_from_rfc3339(&value).map_err(|e| {
                            AppError::ValidationErrors {
                                value: json!({"birth_date": e.to_string()}),
                            }
                        })?;
                        form.birth_date = Some(date.to_utc());
                    }
                    "social_links" => {
                        let value = field.text().await.unwrap_or_default();
                        let links = form.social_links.get_or_insert_with(Vec::new);
                        if !value.is_empty() {
                            links.push(value);
                        }
                    }
                    "image_url" => {
                        form.image_url = FileUpload::try_from_field(field).await?.into();
                    }
                    _ => {}
                }
            }
        }

        form.validate()?;

        Ok(form)
    }
}
async fn update_user(
    State(ctx_state): State<Arc<CtxState>>,
    auth_data: AuthWithLoginAccess,
    mut multipart: Multipart,
) -> CtxResult<Json<UserView>> {
    let form = ProfileSettingsFormInput::try_from_multipart(&mut multipart).await?;
    let local_user_db_service = LocalUserDbService {
        db: &ctx_state.db.client,
        ctx: &auth_data.ctx,
    };
    let mut user = local_user_db_service
        .get_by_id(&auth_data.user_thing_id())
        .await?;

    if let Some(username) = form.username {
        if local_user_db_service
            .get_by_username(&username)
            .await
            .is_ok()
        {
            return Err(AppError::Generic {
                description: "Username is already used".to_string(),
            }
            .into());
        }
        user.username = username.trim().to_string();
    }

    if let Some(full_name) = form.full_name {
        user.full_name = Some(full_name.trim().to_string());
    }

    if let Some(value) = form.birth_date {
        user.birth_date = Some(value.date_naive().to_string());
    }

    if form.social_links.is_some() {
        user.social_links = form.social_links;
    }

    if let Some(file) = form.image_url {
        let result = ctx_state
            .file_storage
            .upload(
                file.data,
                Some(&user.id.clone().unwrap().to_raw().replace(":", "_")),
                &format!("profile_image.{}", file.extension),
                file.content_type.as_deref(),
            )
            .await
            .map_err(|e| AppError::Generic {
                description: e.to_string(),
            })?;

        user.image_uri = Some(result);
    }

    let user = local_user_db_service.update(user).await?;
    Ok(Json(UserView::from(user)))
}

async fn create_post(
    auth_data: AuthWithLoginAccess,
    State(ctx_state): State<Arc<CtxState>>,
    TypedMultipart(input_value): TypedMultipart<PostInput>,
) -> CtxResult<Json<Post>> {
    let user_thing = get_str_thing(&auth_data.user_id)?;
    let disc_id = DiscussionDbService::get_profile_discussion_id(&user_thing);

    let post_service = PostService::new(
        &ctx_state.db.client,
        &auth_data.ctx,
        &ctx_state.event_sender,
        &ctx_state.db.user_notifications,
        &ctx_state.file_storage,
    );

    let post = post_service
        .create(&user_thing.id.to_raw(), &disc_id.to_raw(), input_value)
        .await?;

    Ok(Json(post))
}

async fn get_posts(
    auth_data: AuthWithLoginAccess,
    State(ctx_state): State<Arc<CtxState>>,
    Query(query): Query<DiscussionParams>,
) -> CtxResult<Json<Vec<PostView>>> {
    let user_thing = get_str_thing(&auth_data.user_id)?;
    let disc_id = DiscussionDbService::get_profile_discussion_id(&user_thing);

    let post_service = PostService::new(
        &ctx_state.db.client,
        &auth_data.ctx,
        &ctx_state.event_sender,
        &ctx_state.db.user_notifications,
        &ctx_state.file_storage,
    );

    let posts = post_service
        .get_by_query(
            &disc_id.to_raw(),
            &user_thing.id.to_raw(),
            Pagination {
                order_by: None,
                order_dir: Some(QryOrder::DESC),
                count: query.count.unwrap_or(50),
                start: query.start.unwrap_or(0),
            },
        )
        .await?;

    Ok(Json(posts))
}

#[derive(Deserialize, Serialize, Validate, Debug)]
pub struct SearchInput {
    #[validate(length(min = 3, message = "Min 3 characters"))]
    pub query: String,
}

async fn search_users(
    auth_data: AuthWithLoginAccess,
    State(ctx_state): State<Arc<CtxState>>,
    Query(form_value): Query<SearchInput>,
) -> CtxResult<Json<Vec<UserView>>> {
    let local_user_db_service = LocalUserDbService {
        db: &ctx_state.db.client,
        ctx: &auth_data.ctx,
    };

    let _ = local_user_db_service
        .get_by_id(&auth_data.user_thing_id())
        .await?;

    let items: Vec<UserView> = local_user_db_service
        .search::<UserView>(form_value.query)
        .await?;

    Ok(Json(items))
}

async fn get_user(
    auth_data: AuthWithLoginAccess,
    State(ctx_state): State<Arc<CtxState>>,
) -> CtxResult<Json<UserView>> {
    let local_user_db_service = LocalUserDbService {
        db: &ctx_state.db.client,
        ctx: &auth_data.ctx,
    };

    let thing = get_str_thing(&auth_data.user_id)?;

    let user = local_user_db_service
        .get_view::<UserView>(IdentIdName::Id(thing))
        .await?;

    Ok(Json(user))
}
