use crate::utils::validate_utils::validate_social_links;
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
            authentication_entity::AuthenticationDbService,
            local_user_entity::{LocalUser, LocalUserDbService},
        },
    },
    interfaces::file_storage::FileStorageInterface,
    middleware::{
        ctx::Ctx,
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
    ctx: Ctx,
    State(state): State<Arc<CtxState>>,
    JsonOrFormValidated(data): JsonOrFormValidated<SetPasswordInput>,
) -> CtxResult<Response> {
    let user_id = ctx.user_id()?;

    let user_service = UserService::new(
        LocalUserDbService {
            db: &state.db.client,
            ctx: &ctx,
        },
        &state.email_sender,
        state.verification_code_ttl,
        AuthenticationDbService {
            db: &state.db.client,
            ctx: &ctx,
        },
        &state.db.verification_code,
    );

    user_service.set_password(&user_id, &data.password).await?;

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
    ctx: Ctx,
    State(state): State<Arc<CtxState>>,
    JsonOrFormValidated(data): JsonOrFormValidated<ResetPasswordInput>,
) -> CtxResult<String> {
    let user_id = ctx.user_id()?;

    let user_service = UserService::new(
        LocalUserDbService {
            db: &state.db.client,
            ctx: &ctx,
        },
        &state.email_sender,
        state.verification_code_ttl,
        AuthenticationDbService {
            db: &state.db.client,
            ctx: &ctx,
        },
        &state.db.verification_code,
    );

    let _ = user_service
        .update_password(&user_id, &data.new_password, &data.old_password)
        .await?;

    Ok("Password updated successfully.".to_string())
}

#[derive(Debug, Deserialize, Validate)]
pub struct EmailVerificationStartInput {
    #[validate(email)]
    pub email: String,
}
async fn email_verification_start(
    ctx: Ctx,
    State(ctx_state): State<Arc<CtxState>>,
    JsonOrFormValidated(data): JsonOrFormValidated<EmailVerificationStartInput>,
) -> CtxResult<()> {
    let user_service = UserService::new(
        LocalUserDbService {
            db: &ctx_state.db.client,
            ctx: &ctx,
        },
        &ctx_state.email_sender,
        ctx_state.verification_code_ttl,
        AuthenticationDbService {
            db: &ctx_state.db.client,
            ctx: &ctx,
        },
        &ctx_state.db.verification_code,
    );

    user_service
        .start_email_verification(&ctx.user_thing_id()?, &data.email)
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
    ctx: Ctx,
    State(ctx_state): State<Arc<CtxState>>,
    JsonOrFormValidated(data): JsonOrFormValidated<EmailVerificationConfirmInput>,
) -> CtxResult<()> {
    let user_service = UserService::new(
        LocalUserDbService {
            db: &ctx_state.db.client,
            ctx: &ctx,
        },
        &ctx_state.email_sender,
        ctx_state.verification_code_ttl,
        AuthenticationDbService {
            db: &ctx_state.db.client,
            ctx: &ctx,
        },
        &ctx_state.db.verification_code,
    );

    user_service
        .email_confirmation(&ctx.user_thing_id()?, &data.code, &data.email)
        .await?;

    Ok(())
}

async fn get_following_posts(
    State(state): State<Arc<CtxState>>,
    ctx: Ctx,
) -> CtxResult<Json<Vec<DiscussionPostView>>> {
    let local_user_db_service = LocalUserDbService {
        db: &state.db.client,
        ctx: &ctx,
    };
    let user_id = local_user_db_service.get_ctx_user_thing().await?;
    let data = PostStreamDbService {
        db: &state.db.client,
        ctx: &ctx,
    }
    .get_posts::<DiscussionPostView>(user_id)
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
                        if !value.is_empty() {
                            form.social_links.get_or_insert_with(Vec::new).push(value);
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
    ctx: Ctx,
    mut multipart: Multipart,
) -> CtxResult<Json<LocalUser>> {
    let form = ProfileSettingsFormInput::try_from_multipart(&mut multipart).await?;
    let user_id_id = ctx.user_thing_id()?;
    let local_user_db_service = LocalUserDbService {
        db: &ctx_state.db.client,
        ctx: &ctx,
    };
    let mut user = local_user_db_service.get_by_id(&user_id_id).await?;

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
        user.birth_date = Some(value);
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
            .map_err(|e| {
                ctx.to_ctx_error(AppError::Generic {
                    description: e.to_string(),
                })
            })?;

        user.image_uri = Some(result);
    }

    let user = local_user_db_service.update(user).await?;
    Ok(Json(user))
}

async fn create_post(
    ctx: Ctx,
    State(ctx_state): State<Arc<CtxState>>,
    TypedMultipart(input_value): TypedMultipart<PostInput>,
) -> CtxResult<Json<Post>> {
    let user_thing = get_str_thing(&ctx.user_id()?)?;
    let disc_id = DiscussionDbService::get_profile_discussion_id(&user_thing);

    let post_service = PostService::new(
        &ctx_state.db.client,
        &ctx,
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
    ctx: Ctx,
    State(ctx_state): State<Arc<CtxState>>,
    Query(query): Query<DiscussionParams>,
) -> CtxResult<Json<Vec<PostView>>> {
    let user_thing = get_str_thing(&ctx.user_id()?)?;
    let disc_id = DiscussionDbService::get_profile_discussion_id(&user_thing);

    let post_service = PostService::new(
        &ctx_state.db.client,
        &ctx,
        &ctx_state.event_sender,
        &ctx_state.db.user_notifications,
        &ctx_state.file_storage,
    );

    let posts = post_service
        .get_by_query(&disc_id.to_raw(), &user_thing.id.to_raw(), query)
        .await?;

    Ok(Json(posts))
}

#[derive(Deserialize, Serialize, Validate, Debug)]
pub struct SearchInput {
    #[validate(length(min = 3, message = "Min 3 characters"))]
    pub query: String,
}

async fn search_users(
    ctx: Ctx,
    State(ctx_state): State<Arc<CtxState>>,
    Query(form_value): Query<SearchInput>,
) -> CtxResult<Json<Vec<LocalUser>>> {
    let local_user_db_service = LocalUserDbService {
        db: &ctx_state.db.client,
        ctx: &ctx,
    };

    let _ = local_user_db_service
        .get_by_id(&ctx.user_thing_id()?)
        .await?;

    let items: Vec<LocalUser> = local_user_db_service
        .search(form_value.query)
        .await?
        .into_iter()
        .map(|u| u.into())
        .collect();
    Ok(Json(items))
}
