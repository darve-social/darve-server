use std::sync::Arc;

use askama_axum::axum_core::response::Response;
use askama_axum::Template;
use axum::extract::{DefaultBodyLimit, Path, Query, State};
use axum::response::Html;
use axum::routing::{get, post};
use axum::Router;
use axum_typed_multipart::{FieldData, TryFromMultipart, TypedMultipart};
use serde::{Deserialize, Serialize};
use surrealdb::sql::Thing;
use tempfile::NamedTempFile;
use validator::Validate;

use community::post_entity;
use community_entity::{Community, CommunityDbService};
use discussion_routes::{DiscussionPostView, DiscussionView};
use follow_entity::FollowDbService;
use follow_routes::{UserItemView, UserListView};
use local_user_entity::LocalUserDbService;
use middleware::ctx::Ctx;

use middleware::error::{AppError, CtxResult};
use middleware::mw_ctx::CtxState;
use middleware::utils::db_utils::{
    get_entity_list_view, IdentIdName, UsernameIdent, ViewFieldSelector,
};
use middleware::utils::extractor_utils::{DiscussionParams, JsonOrFormValidated};
use middleware::utils::request_utils::CreatedResponse;
use middleware::utils::string_utils::get_string_thing;
use post_entity::PostDbService;
use post_stream_entity::PostStreamDbService;
use utils::askama_filter_util::filters;
use utils::template_utils::ProfileFormPage;

use super::discussion_routes;
use crate::database::client::Db;
use crate::entities::community::{self, community_entity, post_stream_entity};
use crate::entities::user_auth::authentication_entity::AuthenticationDbService;
use crate::entities::user_auth::{follow_entity, local_user_entity};
use crate::interfaces::file_storage::FileStorageInterface;
use crate::routes::posts::{create_post_entity_route, PostInput};
use crate::routes::user_auth::follow_routes;
use crate::services::user_service::UserService;
use crate::utils::file::convert::convert_field_file_data;
use crate::{middleware, utils};

use utils::validate_utils::empty_string_as_none;
use utils::validate_utils::validate_username;
use crate::database::surrdb_utils::get_thing_id;

pub fn routes(upload_max_size_mb: u64) -> Router<Arc<CtxState>> {
    let max_bytes_val = (1024 * 1024 * upload_max_size_mb) as usize;
    Router::new()
        .route("/u/:username_or_id", get(display_profile))
        .route("/u/following/posts", get(get_following_posts))
        .route("/api/user/:username/posts", get(get_user_posts))
        .route("/api/user/post", post(create_user_post))
        .route("/accounts/edit", get(profile_form))
        .route("/api/accounts/edit", post(profile_save))
        .route("/api/user/search", post(search_users))
        .route(
            "/api/users/current/email/verification/start",
            post(email_verification_start),
        )
        .route(
            "/api/users/current/email/verification/confirm",
            post(email_verification_confirm),
        )
        // the file max limit is set on PostInput property
        .layer(DefaultBodyLimit::max(max_bytes_val))
}

#[derive(Template, TryFromMultipart)]
#[template(path = "nera2/profile_settings_form.html")]
pub struct ProfileSettingsForm {
    pub username: String,
    pub full_name: String,
    pub email: String,
    pub image_url: String,
}

#[derive(Validate, TryFromMultipart, Deserialize, Debug)]
pub struct ProfileSettingsFormInput {
    #[validate(custom(function = "validate_username"))]
    #[serde(deserialize_with = "empty_string_as_none")]
    pub username: Option<String>,

    #[validate(email(message = "Email expected"))]
    #[serde(deserialize_with = "empty_string_as_none")]
    pub email: Option<String>,

    #[validate(length(min = 6, message = "Min 6 character"))]
    #[serde(deserialize_with = "empty_string_as_none")]
    pub full_name: Option<String>,

    #[serde(skip_deserializing)]
    #[form_data(limit = "unlimited")]
    pub image_url: Option<FieldData<NamedTempFile>>,
}
#[derive(Template, Serialize, Deserialize, Debug, Clone)]
#[template(path = "nera2/profile_page.html")]
pub struct ProfilePage {
    theme_name: String,
    window_title: String,
    nav_top_title: String,
    header_title: String,
    footer_text: String,
    pub profile_view: Option<ProfileView>,
}

#[derive(Template, Serialize, Deserialize, Debug, Clone)]
#[template(path = "nera2/profile_view_1.html")]
pub struct ProfileView {
    pub user_id: Thing,
    pub username: String,
    pub full_name: Option<String>,
    pub bio: Option<String>,
    pub image_uri: Option<String>,
    pub social_links: Option<Vec<String>>,
    pub community: Option<Thing>,
    pub profile_discussion: Option<Thing>,
    pub followers_nr: i64,
    pub following_nr: i64,
    pub profile_discussion_view: Option<ProfileDiscussionView>,
}

impl ViewFieldSelector for ProfileView {
    fn get_select_query_fields() -> String {
        "id as user_id, username, full_name, bio, image_uri, social_links, 0 as followers_nr, 0 as following_nr"
            .to_string()
    }
}

#[derive(Template, Serialize, Deserialize, Debug, Clone)]
#[template(path = "nera2/profile_discussion_view_1.html")]
pub struct ProfileDiscussionView {
    id: Option<Thing>,
    pub posts: Vec<ProfilePostView>,
}

impl ViewFieldSelector for ProfileDiscussionView {
    fn get_select_query_fields() -> String {
        "id,  [] as posts".to_string()
    }
}

#[derive(Template, Serialize, Deserialize, Debug, Clone)]
#[template(path = "nera2/profile_post-1-popup.html")]
pub struct ProfilePostView {
    pub id: Thing,
    pub username: Option<String>,
    // belongs_to_id=discussion
    pub belongs_to_id: Thing,
    pub r_title_uri: Option<String>,
    pub title: String,
    pub content: String,
    pub media_links: Option<Vec<String>>,
    pub r_created: String,
    pub replies_nr: i64,
}

impl ViewFieldSelector for ProfilePostView {
    // post fields selct qry for view
    fn get_select_query_fields() -> String {
        "id, created_by.username as username, r_title_uri, title, content, media_links, r_created, belongs_to.id as belongs_to_id, replies_nr".to_string()
    }
}

#[derive(Template, Serialize, Deserialize, Debug)]
#[template(path = "nera2/profile_chat_list.html")]
pub struct ProfileChatList {
    pub user_id: Thing,
    pub discussions: Vec<DiscussionView>,
}

#[derive(Template, Serialize, Deserialize, Debug)]
#[template(path = "nera2/profile_chat.html")]
pub struct ProfileChat {
    user_id: Thing,
    pub discussion: DiscussionView,
}

#[derive(Template, Serialize, Deserialize, Debug)]
#[template(path = "nera2/profile_stream_view.html")]
pub struct FollowingStreamView {
    pub post_list: Vec<DiscussionPostView>,
}

#[derive(Deserialize, Serialize, Validate, Debug)]
pub struct SearchInput {
    #[validate(length(min = 3, message = "Min 3 characters"))]
    pub query: String,
}

async fn profile_form(
    State(ctx_state): State<Arc<CtxState>>,
    ctx: Ctx,
) -> CtxResult<ProfileFormPage> {
    let user = LocalUserDbService {
        db: &ctx_state.db.client,
        ctx: &ctx,
    }
    .get_ctx_user()
    .await?;

    Ok(ProfileFormPage::new(
        Box::new(ProfileSettingsForm {
            username: user.username,
            full_name: "".to_string(),
            email: user.email_verified.unwrap_or_default(),
            image_url: user.image_uri.unwrap_or_default(),
        }),
        None,
        None,
        None,
    ))
}

async fn profile_save(
    State(ctx_state): State<Arc<CtxState>>,
    ctx: Ctx,
    TypedMultipart(body_value): TypedMultipart<ProfileSettingsFormInput>,
) -> CtxResult<Html<String>> {
    body_value.validate().map_err(|e1| {
        ctx.to_ctx_error(AppError::Generic {
            description: e1.to_string(),
        })
    })?;

    let local_user_db_service = LocalUserDbService {
        db: &ctx_state.db.client,
        ctx: &ctx,
    };
    let mut user = local_user_db_service.get_ctx_user().await?;

    if let Some(username) = body_value.username {
        user.username = username.trim().to_string();
    }

    // email needs to be verified first
    // if let Some(email) = body_value.email {
    //     user.email_verified = Some(email.to_string());
    // }

    if let Some(full_name) = body_value.full_name {
        user.full_name = Some(full_name.trim().to_string());
    }

    if let Some(image_file) = body_value.image_url {
        let file = convert_field_file_data(image_file)?;

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
    ctx.to_htmx_or_json(CreatedResponse {
        id: user.id.unwrap().to_raw(),
        uri: Some(user.username),
        success: true,
    })
}

async fn display_profile(
    State(ctx_state): State<Arc<CtxState>>,
    ctx: Ctx,
    Path(username_or_id): Path<String>,
    Query(q_params): Query<DiscussionParams>,
) -> CtxResult<Html<String>> {
    let local_user_db_service = LocalUserDbService {
        db: &ctx_state.db.client,
        ctx: &ctx,
    };
    let is_id = username_or_id.contains(":");
    let user_ident = if !is_id {
        IdentIdName::ColumnIdent {
            column: "username".to_string(),
            val: username_or_id,
            rec: false,
        }
    } else {
        IdentIdName::Id(get_string_thing(username_or_id)?)
    };

    let mut profile_view = local_user_db_service
        .get_view::<ProfileView>(user_ident)
        .await?;
    let profile_comm =
        get_profile_community(&ctx_state.db.client, &ctx, profile_view.user_id.clone()).await?;
    profile_view.community = profile_comm.id;
    profile_view.profile_discussion = profile_comm.default_discussion;

    let disc_id = profile_view.profile_discussion.clone().unwrap();

    let dis_view =
        get_profile_discussion_view(&ctx_state.db.client, &ctx, q_params, disc_id).await?;

    profile_view.profile_discussion_view = Some(dis_view);
    let follow_db_service = FollowDbService {
        db: &ctx_state.db.client,
        ctx: &ctx,
    };
    // TODO cache user follow numbers
    profile_view.following_nr = follow_db_service
        .user_following_number(profile_view.user_id.clone())
        .await?;
    profile_view.followers_nr = follow_db_service
        .user_followers_number(profile_view.user_id.clone())
        .await?;

    ctx.to_htmx_or_json(ProfilePage {
        theme_name: "emerald".to_string(),
        window_title: "win win".to_string(),
        nav_top_title: "navtt".to_string(),
        header_title: "headddr".to_string(),
        footer_text: "foooo".to_string(),
        profile_view: Some(profile_view),
    })
}

async fn get_profile_discussion_view(
    db: &Db,
    ctx: &Ctx,
    q_params: DiscussionParams,
    disc_id: Thing,
) -> CtxResult<ProfileDiscussionView> {
    // let mut dis_view = DiscussionDbService { db: &ctx_state.db.client, ctx: &ctx }.get_view::<ProfileDiscussionView>(IdentIdName::Id(disc_id.clone())).await?;
    let mut dis_view = ProfileDiscussionView {
        id: Some(disc_id.clone()),
        posts: vec![],
    };

    let discussion_posts = PostDbService { db, ctx }
        .get_by_discussion_desc_view::<ProfilePostView>(disc_id.clone(), q_params.clone())
        .await?;
    dis_view.posts = discussion_posts;
    Ok(dis_view)
}

pub async fn get_profile_community(db: &Db, ctx: &Ctx, user_id: Thing) -> CtxResult<Community> {
    let comm_db_ser = CommunityDbService { db, ctx };
    comm_db_ser.get_profile_community(user_id).await
}

// posts user is following
async fn get_following_posts(
    State(state): State<Arc<CtxState>>,
    ctx: Ctx,
) -> CtxResult<Html<String>> {
    let local_user_db_service = LocalUserDbService {
        db: &state.db.client,
        ctx: &ctx,
    };
    let user_id = local_user_db_service.get_ctx_user_thing().await?;
    let stream_post_ids = PostStreamDbService {
        db: &state.db.client,
        ctx: &ctx,
    }
    .user_posts_stream(user_id)
    .await?;
    let post_list = if stream_post_ids.len() > 0 {
        // TODO resolve view access
        get_entity_list_view::<DiscussionPostView>(
            &state.db.client,
            post_entity::TABLE_NAME.to_string(),
            &IdentIdName::Ids(stream_post_ids),
            None,
        )
        .await?
    } else {
        vec![]
    };

    ctx.to_htmx_or_json(FollowingStreamView { post_list })
}

async fn create_user_post(
    ctx: Ctx,
    State(ctx_state): State<Arc<CtxState>>,
    TypedMultipart(input_value): TypedMultipart<PostInput>,
) -> CtxResult<Response> {
    input_value.validate()?;

    let user_id = LocalUserDbService {
        db: &ctx_state.db.client,
        ctx: &ctx,
    }
    .get_ctx_user_thing()
    .await?;
    let profile_comm = get_profile_community(&ctx_state.db.client, &ctx, user_id).await?;

    create_post_entity_route(
        ctx,
        Path(profile_comm.default_discussion.unwrap().to_raw()),
        State(ctx_state),
        TypedMultipart(input_value),
    )
    .await
}

async fn get_user_posts(
    ctx: Ctx,
    Path(username): Path<String>,
    State(ctx_state): State<Arc<CtxState>>,
    Query(q_params): Query<DiscussionParams>,
) -> CtxResult<Html<String>> {
    let local_user_db_service = LocalUserDbService {
        db: &ctx_state.db.client,
        ctx: &ctx,
    };
    let user = local_user_db_service
        .get(UsernameIdent(username).into())
        .await?;
    let profile_comm = get_profile_community(
        &ctx_state.db.client,
        &ctx,
        user.id.expect("user id").clone(),
    )
    .await?;
    let profile_disc_view = get_profile_discussion_view(
        &ctx_state.db.client,
        &ctx,
        q_params,
        profile_comm.default_discussion.expect("profile discussion"),
    )
    .await?;
    ctx.to_htmx_or_json(profile_disc_view)
}

async fn search_users(
    ctx: Ctx,
    State(ctx_state): State<Arc<CtxState>>,
    JsonOrFormValidated(form_value): JsonOrFormValidated<SearchInput>,
) -> CtxResult<Html<String>> {
    let local_user_db_service = LocalUserDbService {
        db: &ctx_state.db.client,
        ctx: &ctx,
    };
    // require logged in
    local_user_db_service.get_ctx_user_thing().await?;
    let items: Vec<UserItemView> = local_user_db_service
        .search(form_value.query)
        .await?
        .into_iter()
        .map(|u| u.into())
        .collect();
    ctx.to_htmx_or_json(UserListView { items })
}

#[derive(Debug, Deserialize, Validate, Serialize)]
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

    let current_user_thing_id = ctx.user_id()?;
    let user_id = get_thing_id(&current_user_thing_id);
    
    user_service
        .start_email_verification(user_id, &data.email)
        .await?;

    Ok(())
}

#[derive(Debug, Deserialize, Validate, Serialize)]

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

    let u_thing_str = ctx.user_id()?;
    let user_id = get_thing_id(&u_thing_str);
    
    user_service
        .email_confirmation(user_id, &data.code, &data.email)
        .await?;

    Ok(())
}
