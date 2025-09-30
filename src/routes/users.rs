use crate::{
    entities::{
        community::post_entity::{PostDbService, PostType},
        nickname::Nickname,
        user_auth::local_user_entity::UpdateUser,
    },
    interfaces::repositories::{
        discussion_user::DiscussionUserRepositoryInterface, nickname::NicknamesRepositoryInterface,
    },
    middleware::{
        auth_with_login_access::AuthWithLoginAccess,
        utils::{
            db_utils::{IdentIdName, Pagination},
            string_utils::get_str_thing,
        },
    },
    models::view::{discussion_user::DiscussionUserView, post::PostView, user::UserView},
    utils::validate_utils::validate_social_links,
};

use axum::{
    extract::{DefaultBodyLimit, Multipart, Path as ExPath, Query, State},
    response::{IntoResponse, Response},
    routing::{get, patch, post},
    Json, Router,
};
use axum_extra::extract::Query as ExtractQuery;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::{path::Path, sync::Arc};
use validator::Validate;

use crate::{
    entities::user_auth::{
        authentication_entity::AuthenticationDbService, local_user_entity::LocalUserDbService,
    },
    middleware::{
        error::{AppError, CtxResult},
        mw_ctx::CtxState,
        utils::extractor_utils::JsonOrFormValidated,
    },
    services::user_service::UserService,
    utils::{self, file::convert::FileUpload},
};

use utils::validate_utils::validate_birth_date;
use utils::validate_utils::validate_username;

pub fn routes() -> Router<Arc<CtxState>> {
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
        .route("/api/users/current/latest_posts", get(get_latest_posts))
        .route(
            "/api/users/current/email/verification/start",
            post(email_verification_start),
        )
        .route(
            "/api/users/current/email/verification/confirm",
            post(email_verification_confirm),
        )
        .route(
            "/api/users/current",
            patch(update_user).layer(DefaultBodyLimit::max(1024 * 1024 * 8)),
        )
        .route("/api/users/current", get(get_user))
        .route("/api/users", get(search_users))
        .route("/api/users/status", get(get_users_status))
        .route("/api/users/{user_id}/nickname", post(set_nickname))
        .route("/api/users/current/nicknames", get(get_nicknames))
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
        state.email_sender.clone(),
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
        state.email_sender.clone(),
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
        state.email_sender.clone(),
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
        state.email_sender.clone(),
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
        ctx_state.email_sender.clone(),
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
        ctx_state.email_sender.clone(),
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

#[derive(Debug, Deserialize)]
pub struct GetFollowerPostsQuery {
    pub r#type: Option<PostType>,
    pub start: Option<u32>,
    pub count: Option<u16>,
}

async fn get_following_posts(
    auth_data: AuthWithLoginAccess,
    State(state): State<Arc<CtxState>>,
    Query(query): Query<GetFollowerPostsQuery>,
) -> CtxResult<Json<Vec<PostView>>> {
    let local_user_db_service = LocalUserDbService {
        db: &state.db.client,
        ctx: &auth_data.ctx,
    };
    let _ = local_user_db_service
        .get_by_id(&auth_data.user_thing_id())
        .await?;

    let post_db_service = PostDbService {
        db: &state.db.client,
        ctx: &auth_data.ctx,
    };

    let types = query
        .r#type
        .map_or(vec![PostType::Idea, PostType::Public], |v| vec![v]);

    let pag = Pagination {
        order_by: None,
        order_dir: None,
        count: query.count.unwrap_or(50),
        start: query.start.unwrap_or(0),
    };
    let posts = post_db_service
        .get_by_followers(&auth_data.user_thing_id(), types, pag)
        .await?;

    Ok(Json(posts))
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

    let user = local_user_db_service
        .get_by_id(&auth_data.user_thing_id())
        .await?;

    let mut update_user = UpdateUser {
        username: None,
        full_name: None,
        birth_date: None,
        phone: None,
        bio: None,
        social_links: None,
        image_uri: None,
        is_otp_enabled: None,
        otp_secret: None,
    };
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
        update_user.username = Some(Some(username.trim().to_string()));
    }

    match form.full_name {
        UpdateField::Set(value) => update_user.full_name = Some(Some(value.trim().to_string())),
        UpdateField::Unset => update_user.full_name = Some(None),
        _ => (),
    };

    match form.bio {
        UpdateField::Set(value) => update_user.bio = Some(Some(value.trim().to_string())),
        UpdateField::Unset => update_user.bio = Some(None),
        _ => (),
    };

    match form.birth_date {
        UpdateField::Set(value) => {
            update_user.birth_date = Some(Some(value.date_naive().to_string()))
        }
        UpdateField::Unset => update_user.birth_date = Some(None),
        _ => (),
    };

    if form.social_links.is_some() {
        update_user.social_links = form.social_links;
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

            update_user.image_uri = Some(Some(result));
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
            update_user.image_uri = Some(None);
        }
        _ => (),
    };
    let user = local_user_db_service
        .update(user.id.as_ref().unwrap().id.to_raw().as_str(), update_user)
        .await?;

    Ok(Json(UserView::from(user)))
}

#[derive(Deserialize, Serialize, Validate, Debug)]
pub struct SearchInput {
    #[validate(length(min = 3, message = "Min 3 characters"))]
    pub query: String,
    pub start: Option<u32>,
    pub count: Option<u16>,
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
        .search::<UserView>(
            form_value.query,
            Pagination {
                order_by: None,
                order_dir: None,
                count: form_value.count.unwrap_or(50),
                start: form_value.start.unwrap_or(0),
            },
        )
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

#[derive(Debug, Deserialize)]
struct GetPostsQuery {
    start: Option<u32>,
    count: Option<u16>,
}

async fn get_latest_posts(
    auth_data: AuthWithLoginAccess,
    State(state): State<Arc<CtxState>>,
    Query(query): Query<GetPostsQuery>,
) -> CtxResult<Json<Vec<DiscussionUserView>>> {
    let user_db_service = LocalUserDbService {
        db: &state.db.client,
        ctx: &auth_data.ctx,
    };
    let user = user_db_service
        .get_by_id(&auth_data.user_thing_id())
        .await?;

    let pagination = Pagination {
        order_by: None,
        order_dir: None,
        count: query.count.unwrap_or(20),
        start: query.start.unwrap_or(0),
    };

    let data = state
        .db
        .discussion_users
        .get_by_user::<DiscussionUserView>(&user.id.as_ref().unwrap().id.to_raw(), pagination, true)
        .await?;

    Ok(Json(data))
}

#[derive(Debug, Serialize)]
struct UserStatus {
    user_id: String,
    is_online: bool,
}

#[derive(Debug, Deserialize)]
struct GetUsersStatusQuery {
    #[serde(default)]
    user_ids: Vec<String>,
}

async fn get_users_status(
    auth_data: AuthWithLoginAccess,
    State(state): State<Arc<CtxState>>,
    ExtractQuery(query): ExtractQuery<GetUsersStatusQuery>,
) -> CtxResult<Json<Vec<UserStatus>>> {
    if query.user_ids.len() > 50 {
        return Err(AppError::Generic {
            description: "To much ids".to_string(),
        }
        .into());
    }

    let _ = LocalUserDbService {
        db: &state.db.client,
        ctx: &auth_data.ctx,
    }
    .get_by_id(&auth_data.user_thing_id())
    .await?;

    let users_status = query
        .user_ids
        .into_iter()
        .map(|id| UserStatus {
            is_online: state.online_users.get(&id).is_some(),
            user_id: id,
        })
        .collect::<Vec<UserStatus>>();

    Ok(Json(users_status))
}

#[derive(Debug, Deserialize)]
struct SetNicknameData {
    nickname: Option<String>,
}

async fn set_nickname(
    auth_data: AuthWithLoginAccess,
    ExPath(user_id): ExPath<String>,
    State(state): State<Arc<CtxState>>,
    Json(body): Json<SetNicknameData>,
) -> CtxResult<()> {
    let user_db_service = LocalUserDbService {
        db: &state.db.client,
        ctx: &auth_data.ctx,
    };
    let current_user = user_db_service
        .get_by_id(&auth_data.user_thing_id())
        .await?;

    let to_user = user_db_service.get_by_id(&user_id).await?;

    let to_user_id = to_user.id.as_ref().unwrap().id.to_raw();
    let current_user_id = current_user.id.as_ref().unwrap().id.to_raw();

    match body.nickname {
        Some(value) => {
            if value.trim().is_empty() {
                return Err(AppError::Generic {
                    description: "The nickname must not be empty".into(),
                }
                .into());
            }
            state
                .db
                .nicknames
                .upsert(&current_user_id, &to_user_id, value)
                .await?
        }
        None => {
            state
                .db
                .nicknames
                .remove(&current_user_id, &to_user_id)
                .await?
        }
    }
    Ok(())
}

async fn get_nicknames(
    auth_data: AuthWithLoginAccess,
    State(state): State<Arc<CtxState>>,
) -> CtxResult<Json<Vec<Nickname>>> {
    let user_db_service = LocalUserDbService {
        db: &state.db.client,
        ctx: &auth_data.ctx,
    };
    let current_user = user_db_service
        .get_by_id(&auth_data.user_thing_id())
        .await?;

    let nicknames = state
        .db
        .nicknames
        .get_by_user(current_user.id.as_ref().unwrap().id.to_raw().as_str())
        .await?;

    Ok(Json(nicknames))
}
