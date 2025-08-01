use crate::{
    middleware::{auth_with_login_access::AuthWithLoginAccess, utils::db_utils::IdentIdName},
    models::view::UserView,
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
use std::{path::Path, sync::Arc};
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
        .route(
            "/api/users/current/set_password/start",
            post(start_set_password),
        )
        .route(
            "/api/users/current/set_password/confirm",
            post(set_password),
        )
        .route(
            "/api/users/current/update_password/start",
            post(start_update_password),
        )
        .route(
            "/api/users/current/update_password/confirm",
            post(reset_password),
        )
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
    pub password: String,
    #[validate(length(min = 6, message = "Min 6 characters"))]
    pub code: String,
}

async fn start_set_password(
    auth_data: AuthWithLoginAccess,
    State(state): State<Arc<CtxState>>,
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
        .start_set_password(&auth_data.user_thing_id())
        .await?;

    Ok(().into_response())
}

async fn start_update_password(
    auth_data: AuthWithLoginAccess,
    State(state): State<Arc<CtxState>>,
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
        .start_update_password(&auth_data.user_thing_id())
        .await?;

    Ok(().into_response())
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
        .set_password(&auth_data.user_thing_id(), &data.password, &data.code)
        .await?;

    Ok(().into_response())
}

#[derive(Debug, Deserialize, Validate)]
struct ResetPasswordInput {
    #[validate(length(min = 6, message = "Min 6 characters"))]
    old_password: String,
    #[validate(length(min = 6, message = "Min 6 characters"))]
    new_password: String,
    #[validate(length(min = 6, message = "Min 6 characters"))]
    pub code: String,
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
        .update_password(
            &auth_data.user_thing_id(),
            &data.new_password,
            &data.old_password,
            &data.code,
        )
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

#[derive(Debug)]
enum UpdateField<T> {
    Set(T),
    Unset,
    None,
}

#[derive(Validate, Debug)]
pub struct ProfileSettingsFormInput {
    username: Option<String>,
    full_name: UpdateField<String>,
    bio: UpdateField<String>,
    image_url: UpdateField<FileUpload>,
    social_links: Option<Vec<String>>,
    birth_date: UpdateField<DateTime<Utc>>,
}

impl ProfileSettingsFormInput {
    async fn try_from_multipart(multipart: &mut Multipart) -> CtxResult<Self> {
        let mut form = ProfileSettingsFormInput {
            username: None,
            full_name: UpdateField::None,
            image_url: UpdateField::None,
            social_links: None,
            birth_date: UpdateField::None,
            bio: UpdateField::None,
        };

        while let Some(field) = multipart.next_field().await.unwrap() {
            if let Some(name) = field.name() {
                match name {
                    "username" => {
                        let username = field.text().await.unwrap_or_default();
                        validate_username(&username).map_err(|e| AppError::ValidationErrors {
                            value: json!({ "username": e.message }),
                        })?;

                        form.username = Some(username);
                    }
                    "bio" => {
                        let bio = field.text().await.unwrap_or_default();
                        form.bio = if !bio.is_empty() {
                            UpdateField::Set(bio)
                        } else {
                            UpdateField::Unset
                        }
                    }
                    "full_name" => {
                        let full_name = field.text().await.unwrap_or_default();
                        form.full_name = if !full_name.is_empty() {
                            if full_name.trim().len() < 6 {
                                return Err(AppError::ValidationErrors {
                                    value: json!({ "full_name": "Min 6 characters"}),
                                }
                                .into());
                            };

                            UpdateField::Set(full_name)
                        } else {
                            UpdateField::Unset
                        }
                    }
                    "birth_date" => {
                        let value: String = field.text().await.unwrap_or_default();
                        form.birth_date = if !value.is_empty() {
                            let date = DateTime::parse_from_rfc3339(&value)
                                .map_err(|e| AppError::ValidationErrors {
                                    value: json!({"birth_date": e.to_string()}),
                                })?
                                .to_utc();
                            validate_birth_date(&date).map_err(|e| AppError::ValidationErrors {
                                value: json!({"birth_date": e.message}),
                            })?;
                            UpdateField::Set(date)
                        } else {
                            UpdateField::Unset
                        }
                    }
                    "social_links" => {
                        let value = field.text().await.unwrap_or_default();
                        let links = form.social_links.get_or_insert_with(Vec::new);
                        if !value.is_empty() {
                            links.push(value);
                        }
                        validate_social_links(&links).map_err(|e| AppError::ValidationErrors {
                            value: json!({ "social_links": e.message }),
                        })?;
                    }
                    "image_url" => {
                        let file_name = field.file_name().map(|v| v.to_string());
                        let content_type = field.content_type().map(|v| v.to_string());
                        let data = field.bytes().await.map_err(|e| AppError::Generic {
                            description: format!("Failed to read file: {}", e),
                        })?;

                        form.image_url = if !data.is_empty() {
                            let file_name = file_name.ok_or(AppError::Generic {
                                description: "Missing file name".to_string(),
                            })?;
                            let extension = Path::new(&file_name)
                                .extension()
                                .and_then(|e| e.to_str())
                                .ok_or(AppError::Generic {
                                    description: "File has no valid extension".to_string(),
                                })?
                                .to_string();

                            UpdateField::Set(FileUpload {
                                content_type,
                                file_name,
                                data: data.to_vec(),
                                extension,
                            })
                        } else {
                            UpdateField::Unset
                        }
                    }
                    _ => {}
                }
            }
        }
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

    match form.full_name {
        UpdateField::Set(value) => user.full_name = Some(value.trim().to_string()),
        UpdateField::Unset => user.full_name = None,
        _ => (),
    };

    match form.bio {
        UpdateField::Set(value) => user.bio = Some(value.trim().to_string()),
        UpdateField::Unset => user.bio = None,
        _ => (),
    };

    match form.birth_date {
        UpdateField::Set(value) => user.birth_date = Some(value.date_naive().to_string()),
        UpdateField::Unset => user.birth_date = None,
        _ => (),
    };

    if form.social_links.is_some() {
        user.social_links = form.social_links;
    }

    match form.image_url {
        UpdateField::Set(file) => {
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
        UpdateField::Unset => {
            if let Some(url) = user.image_uri {
                let _ = ctx_state
                    .file_storage
                    .delete(
                        Some(&user.id.clone().unwrap().to_raw().replace(":", "_")),
                        url.split("/").last().unwrap(),
                    )
                    .await;
            }
            user.image_uri = None;
        }
        _ => (),
    };
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
