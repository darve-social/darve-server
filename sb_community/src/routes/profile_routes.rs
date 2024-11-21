use crate::routes::post_routes::UPLOADS_URL_BASE;
use std::io::Write;
use std::str::FromStr;

use askama_axum::axum_core::extract::DefaultBodyLimit;
use askama_axum::axum_core::response::IntoResponse;
use askama_axum::Template;
use axum::extract::{Path, State};
use axum::response::Html;
use axum::routing::{get, post};
use axum::Router;
use axum_typed_multipart::{FieldData, TryFromMultipart, TypedMultipart};
use futures::TryFutureExt;
use serde::{Deserialize, Serialize};
use std::path::Path as FPath;
use surrealdb::sql::Thing;
use tempfile::NamedTempFile;
use validator::Validate;

use crate::entity::community_entitiy::{Community, CommunityDbService};
use crate::entity::discussion_entitiy::{Discussion, DiscussionDbService};
use crate::entity::post_entitiy::PostDbService;
use crate::routes::community_routes::{create_update_community, CommunityInput};
use crate::routes::discussion_routes::{SseEventName};
use sb_middleware::ctx::Ctx;
use sb_middleware::db::Db;
use sb_middleware::error::{AppError, CtxResult};
use sb_middleware::mw_ctx::CtxState;
use sb_middleware::utils::db_utils::{get_entities_by_id, record_exists, IdentIdName, ViewFieldSelector};
use sb_middleware::utils::extractor_utils::DiscussionParams;
use sb_middleware::utils::request_utils::CreatedResponse;
use sb_middleware::utils::string_utils::get_string_thing;
use sb_user_auth::entity::local_user_entity::LocalUserDbService;
use sb_user_auth::utils::askama_filter_util::filters;
use sb_user_auth::utils::template_utils::ProfileFormPage;

pub fn routes(state: CtxState) -> Router {
    Router::new()
        .route("/u/:username", get(display_profile))
        .route("/accounts/edit", get(profile_form))
        .route("/api/accounts/edit", post(profile_save))
        .route("/api/user_chat/list", get(get_chats))
        .route("/api/user_chat/with/:other_user_id", get(get_chat_discussion))
        .layer(DefaultBodyLimit::max(1024 * 1024 * 1))
        .with_state(state)
}

#[derive(Template, TryFromMultipart)]
#[template(path = "nera2/profile_settings_form.html")]
pub struct ProfileSettingsForm {
    pub username: String,
    pub full_name: String,
    pub email: String,
    pub image_url: String,

}

#[derive(Validate, TryFromMultipart)]
pub struct ProfileSettingsFormInput {
    #[validate(length(min = 5, message = "Min 5 characters"))]
    pub username: String,
    #[validate(email(message = "Email expected"))]
    pub email: String,
    #[form_data(limit = "1MiB")]
    pub image_url: Option<FieldData<NamedTempFile>>,
}

#[derive(Template, Serialize, Deserialize, Debug)]
#[template(path = "nera2/profile_page.html")]
pub struct ProfilePage {
    theme_name: String,
    window_title: String,
    nav_top_title: String,
    header_title: String,
    footer_text: String,
    pub(crate) profile_view: Option<ProfileView>,
}

#[derive(Template, Serialize, Deserialize, Debug)]
#[template(path = "nera2/profile_view_1.html")]
pub struct ProfileView {
    user_id: Thing,
    community: Option<Thing>,
    profile_discussion: Option<Thing>,
    pub(crate) profile_discussion_view: Option<ProfileDiscussionView>,
}

impl ViewFieldSelector for ProfileView {
    fn get_select_query_fields(_ident: &IdentIdName) -> String {
        // "id as user_id, community, community.main_discussion as profile_discussion".to_string()
        "id as user_id".to_string()
    }
}

#[derive(Template, Serialize, Deserialize, Debug)]
#[template(path = "nera2/profile_discussion_view_1.html")]
pub struct ProfileDiscussionView {
    id: Option<Thing>,
    pub(crate) posts: Vec<ProfilePostView>,
}

impl ViewFieldSelector for ProfileDiscussionView {
    fn get_select_query_fields(_ident: &IdentIdName) -> String {
        "id,  [] as posts".to_string()
    }
}

#[derive(Template, Serialize, Deserialize, Debug)]
#[template(path = "nera2/profile_post-1-popup.html")]
pub struct ProfilePostView {
    pub id: Thing,
    pub username: Option<String>,
    pub discussion_id: Thing,
    pub r_title_uri: Option<String>,
    pub title: String,
    pub content: String,
    pub r_created: String,
    pub replies_nr: i64,
}

impl ViewFieldSelector for ProfilePostView {
    // post fields selct qry for view
    fn get_select_query_fields(_ident: &IdentIdName) -> String {
        "id, created_by.username as username, r_title_uri, title, content, r_created, discussion.id as discussion_id, replies_nr".to_string()
    }
}

#[derive(Template, Serialize, Deserialize, Debug)]
#[template(path = "nera2/profile_chat_list.html")]
pub struct ProfileChatList {
    pub user_id: Thing,
    pub discussions: Vec<Discussion>,
}

#[derive(Template, Serialize, Deserialize, Debug)]
#[template(path = "nera2/profile_chat.html")]
pub struct ProfileChat {
    user_id: Thing,
    pub discussion: Discussion,
}

async fn profile_form(
    State(ctx_state): State<CtxState>,
    ctx: Ctx,
) -> CtxResult<ProfileFormPage> {
    let user = LocalUserDbService { db: &ctx_state._db, ctx: &ctx }.get_ctx_user().await?;

    Ok(ProfileFormPage::new(Box::new(ProfileSettingsForm { username: user.username, full_name: "".to_string(), email: user.email.unwrap_or_default(), image_url: user.image_uri.unwrap_or_default() }), None, None))
}

async fn profile_save(
    State(ctx_state): State<CtxState>,
    ctx: Ctx,
    TypedMultipart(body_value): TypedMultipart<ProfileSettingsFormInput>,
) -> CtxResult<Html<String>> {
    let mut user = LocalUserDbService { db: &ctx_state._db, ctx: &ctx }.get_ctx_user().await?;

    let local_user_db_service = LocalUserDbService { db: &ctx_state._db, ctx: &ctx };
    body_value.validate().map_err(|e1| ctx.to_ctx_error(AppError::Generic { description: e1.to_string() }))?;
    user.email = if body_value.email.trim().len() > 0 {
        Some(body_value.email.trim().to_string())
    } else { user.email };

    user.username = if body_value.username.trim().len() > 0 {
        body_value.username.trim().to_string()
    } else { user.username };

    if let Some(image_file) = body_value.image_url {
        let file_name = image_file.metadata.file_name.unwrap();
        let ext = file_name.split(".").last().ok_or(AppError::Generic { description: "File has no extension".to_string() })?;

        let file_name = format!("uid_{}-profile_image.{ext}", user.id.clone().unwrap().id.to_raw());
        let path = FPath::new(&ctx_state.uploads_dir).join(file_name.as_str());
        let saved = image_file.contents.persist(path.clone());
        if saved.is_ok() {
            user.image_uri = Some(format!("{UPLOADS_URL_BASE}/{file_name}"));
        }
    }

    let user = local_user_db_service.update(user).await?;
    ctx.to_htmx_or_json_res(CreatedResponse { id: user.id.unwrap().to_raw(), uri: Some(user.username), success: true })
}

async fn display_profile(
    State(ctx_state): State<CtxState>,
    ctx: Ctx,
    Path(username): Path<String>,
    q_params: DiscussionParams,
) -> CtxResult<ProfilePage> {
    let local_user_db_service = LocalUserDbService { db: &ctx_state._db, ctx: &ctx };
    let mut profile_view = local_user_db_service
        .get_view::<ProfileView>(IdentIdName::ColumnIdent { column: "username".to_string(), val: username, rec: false }).await?;
    let profile_comm = get_profile_community(&ctx_state._db, &ctx, profile_view.user_id.clone()).await?;

    profile_view.community = profile_comm.id;
    profile_view.profile_discussion = profile_comm.main_discussion;

    let disc_id = profile_view.profile_discussion.clone().unwrap();
    let mut dis_view = DiscussionDbService { db: &ctx_state._db, ctx: &ctx }.get_view::<ProfileDiscussionView>(IdentIdName::Id(disc_id.clone())).await?;

    let discussion_posts = PostDbService { db: &ctx_state._db, ctx: &ctx }
        .get_by_discussion_desc_view::<ProfilePostView>(disc_id.clone(), q_params.clone()).await?;
    dis_view.posts = discussion_posts;

    profile_view.profile_discussion_view = Some(dis_view);

    Ok(ProfilePage {
        theme_name: "emerald".to_string(),
        window_title: "win win".to_string(),
        nav_top_title: "navtt".to_string(),
        header_title: "headddr".to_string(),
        footer_text: "foooo".to_string(),
        profile_view: Some(profile_view),
    })
}

async fn get_profile_community(db: &Db, ctx: &Ctx, user_id: Thing) -> CtxResult<Community> {
    let comm_db_ser = CommunityDbService { db, ctx };
    let profile_comm_id = CommunityDbService::get_profile_community_id(user_id.clone());
    match comm_db_ser.get(IdentIdName::Id(profile_comm_id.clone())).await {
        Ok(comm) => Ok(comm),
        Err(err) => {
            match err.error {
                AppError::EntityFailIdNotFound { .. } =>
                    create_update_community(db, ctx, CommunityInput { id: profile_comm_id.to_raw(), create_custom_id: Some(true), name_uri: user_id.to_raw(), title: user_id.to_raw() }, &user_id).await,
                _ => Err(err)
            }
        }
    }
}

async fn get_chats(State(CtxState { _db, .. }): State<CtxState>,
                   ctx: Ctx,
) -> CtxResult<Html<String>> {
    println!("->> {:<12} - get_chats", "HANDLER");
    let local_user_db_service = LocalUserDbService { db: &_db, ctx: &ctx };
    let user_id = local_user_db_service.get_ctx_user_thing().await?;
    let comm = get_profile_community(&_db, &ctx, user_id.clone()).await?;
    let discussion_ids = comm.profile_chats.unwrap_or(vec![]);
    let discussions = get_entities_by_id::<Discussion>(&_db, discussion_ids).await?;
    ctx.to_htmx_or_json_res(ProfileChatList { user_id, discussions })
}

async fn get_chat_discussion(State(CtxState { _db, .. }): State<CtxState>,
                             ctx: Ctx,
                             Path(other_user_id): Path<String>,
) -> CtxResult<Html<String>> {
    println!("->> {:<12} - get get_chat_discussion", "HANDLER");
    let local_user_db_service = LocalUserDbService { db: &_db, ctx: &ctx };
    let user_id = local_user_db_service.get_ctx_user_thing().await?;
    let other_user_id = get_string_thing(other_user_id)?;
    // TODO limit nr of requests or count them to distinguish bots for user ids
    local_user_db_service.exists(IdentIdName::Id(other_user_id.clone())).await?;

    let comm = get_profile_community(&_db, &ctx, user_id.clone()).await?;
    let discussions = comm.profile_chats.clone().unwrap_or(vec![]);
    let comm_db_service = CommunityDbService { db: &_db, ctx: &ctx };
    let discussion_db_service = DiscussionDbService { db: &_db, ctx: &ctx };
    let existing = discussion_db_service.get_chatroom_with_users(discussions, vec![user_id.clone(), other_user_id.clone()]).await?;
    let discussion = match existing {
        None => create_chat_discussion(user_id.clone(), other_user_id, comm, comm_db_service, discussion_db_service).await?,
        Some(disc) => disc
    };
    ctx.to_htmx_or_json_res(ProfileChat { discussion, user_id })
}

async fn create_chat_discussion<'a>(user_id: Thing, other_user_id: Thing, comm: Community, comm_db_service: CommunityDbService<'a>, discussion_db_service: DiscussionDbService<'a>) -> CtxResult<Discussion> {
    let disc = discussion_db_service.create_update(Discussion {
        id: None,
        belongs_to: comm.id.unwrap(),
        title: None,
        topics: None,
        chat_room_user_ids: Some(vec![user_id.clone(), other_user_id.clone()]),
        r_created: None,
        created_by: user_id.clone(),
    }).await?;
    let exists = record_exists(comm_db_service.db, CommunityDbService::get_profile_community_id(user_id.clone())).await;
    if exists.is_err() {
        // creates profile community
        get_profile_community(comm_db_service.db, comm_db_service.ctx, user_id.clone()).await?;
    }
    let exists = record_exists(comm_db_service.db, CommunityDbService::get_profile_community_id(other_user_id.clone())).await;
    if exists.is_err() {
        // creates profile community
        get_profile_community(comm_db_service.db, comm_db_service.ctx, other_user_id.clone()).await?;
    }

    comm_db_service.add_profile_chat_discussion(user_id, disc.id.clone().unwrap()).await?;
    comm_db_service.add_profile_chat_discussion(other_user_id, disc.id.clone().unwrap()).await?;
    Ok(disc)
}
