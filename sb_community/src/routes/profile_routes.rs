use crate::routes::post_routes::{create_post_entity_route, PostInput, UPLOADS_URL_BASE};
use askama_axum::axum_core::extract::DefaultBodyLimit;
use askama_axum::axum_core::response::Response;
use askama_axum::Template;
use axum::extract::{Path, State};
use axum::response::sse::Event;
use axum::response::{Html, Sse};
use axum::routing::{get, post};
use axum::Router;
use axum_typed_multipart::{FieldData, TryFromMultipart, TypedMultipart};
use serde::{Deserialize, Serialize};
use std::path::Path as FPath;
use std::string::ToString;
use surrealdb::sql::Thing;
use tempfile::NamedTempFile;
use validator::Validate;

use crate::entity::community_entitiy::{Community, CommunityDbService};
use crate::entity::discussion_entitiy::{Discussion, DiscussionDbService};
use crate::entity::post_entitiy::PostDbService;
use crate::entity::post_stream_entitiy::PostStreamDbService;
use crate::routes::discussion_routes::{
    get_discussion_view, DiscussionLatestPostView, DiscussionPostView, DiscussionView, SseEventName,
};
use futures::stream::Stream as FStream;
use once_cell::sync::Lazy;
use sb_middleware::ctx::Ctx;
use sb_middleware::db::Db;
use sb_middleware::error::{AppError, CtxResult};
use sb_middleware::mw_ctx::CtxState;
use sb_middleware::utils::db_utils::{
    get_entity_list_view, record_exists, IdentIdName, UsernameIdent, ViewFieldSelector,
};
use sb_middleware::utils::extractor_utils::{DiscussionParams, JsonOrFormValidated};
use sb_middleware::utils::request_utils::CreatedResponse;
use sb_middleware::utils::string_utils::get_string_thing;
use sb_user_auth::entity::follow_entitiy::FollowDbService;
use sb_user_auth::entity::local_user_entity::LocalUserDbService;
use sb_user_auth::entity::user_notification_entitiy::{UserNotification, UserNotificationEvent};
use sb_user_auth::routes::follow_routes::{UserItemView, UserListView};
use sb_user_auth::routes::user_notification_routes::create_user_notifications_sse;
use sb_user_auth::utils::askama_filter_util::filters;
use sb_user_auth::utils::template_utils::ProfileFormPage;

pub fn routes(state: CtxState) -> Router {
    // let max_bytes_val = (1024 * 1024 * state.upload_max_size_mb) as usize;
    Router::new()
        .route("/u/:username_or_id", get(display_profile))
        .route("/u/following/posts", get(get_following_posts))
        .route("/api/user/:username/posts", get(get_user_posts))
        .route("/api/user/post", post(create_user_post))
        .route("/accounts/edit", get(profile_form))
        .route("/api/accounts/edit", post(profile_save))
        .route("/api/user_chat/list", get(get_chats))
        .route("/api/user_chat/list/sse", get(get_chats_sse))
        .route("/api/user/search", post(search_users))
        .route(
            "/api/user_chat/with/:other_user_id",
            get(get_create_chat_discussion),
        )
        // the file max limit is set on PostInput property
        // .layer(DefaultBodyLimit::max(max_bytes_val))
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
    // #[validate(length(min = 5, message = "Min 5 characters"))]
    pub username: String,
    // #[validate(email(message = "Email expected"))]
    pub email: String,
    pub full_name: String,
    #[form_data(limit = "5MiB")]
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
    fn get_select_query_fields(_ident: &IdentIdName) -> String {
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
    fn get_select_query_fields(_ident: &IdentIdName) -> String {
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
    fn get_select_query_fields(_ident: &IdentIdName) -> String {
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

async fn profile_form(State(ctx_state): State<CtxState>, ctx: Ctx) -> CtxResult<ProfileFormPage> {
    let user = LocalUserDbService {
        db: &ctx_state._db,
        ctx: &ctx,
    }
    .get_ctx_user()
    .await?;

    Ok(ProfileFormPage::new(
        Box::new(ProfileSettingsForm {
            username: user.username,
            full_name: "".to_string(),
            email: user.email.unwrap_or_default(),
            image_url: user.image_uri.unwrap_or_default(),
        }),
        None,
        None,
        None,
    ))
}

async fn profile_save(
    State(ctx_state): State<CtxState>,
    ctx: Ctx,
    TypedMultipart(body_value): TypedMultipart<ProfileSettingsFormInput>,
) -> CtxResult<Html<String>> {
    let mut user = LocalUserDbService {
        db: &ctx_state._db,
        ctx: &ctx,
    }
    .get_ctx_user()
    .await?;

    let local_user_db_service = LocalUserDbService {
        db: &ctx_state._db,
        ctx: &ctx,
    };
    body_value.validate().map_err(|e1| {
        ctx.to_ctx_error(AppError::Generic {
            description: e1.to_string(),
        })
    })?;
    user.email = if body_value.email.trim().len() > 0 {
        Some(body_value.email.trim().to_string())
    } else {
        user.email
    };
    user.full_name = if body_value.full_name.trim().len() > 0 {
        Some(body_value.full_name.trim().to_string())
    } else {
        user.full_name
    };

    user.username = if body_value.username.trim().len() > 0 {
        body_value.username.trim().to_string()
    } else {
        user.username
    };

    if let Some(image_file) = body_value.image_url {
        let file_name = image_file.metadata.file_name.unwrap();
        let ext = file_name.split(".").last().ok_or(AppError::Generic {
            description: "File has no extension".to_string(),
        })?;

        let file_name = format!(
            "uid_{}-profile_image.{ext}",
            user.id.clone().unwrap().id.to_raw()
        );
        let path = FPath::new(&ctx_state.uploads_dir).join(file_name.as_str());
        let saved = image_file.contents.persist(path.clone());
        if saved.is_ok() {
            user.image_uri = Some(format!("{UPLOADS_URL_BASE}/{file_name}"));
        }
    }

    let user = local_user_db_service.update(user).await?;
    ctx.to_htmx_or_json(CreatedResponse {
        id: user.id.unwrap().to_raw(),
        uri: Some(user.username),
        success: true,
    })
}

async fn display_profile(
    State(ctx_state): State<CtxState>,
    ctx: Ctx,
    Path(username_or_id): Path<String>,
    q_params: DiscussionParams,
) -> CtxResult<Html<String>> {
    let local_user_db_service = LocalUserDbService {
        db: &ctx_state._db,
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
        get_profile_community(&ctx_state._db, &ctx, profile_view.user_id.clone()).await?;
    profile_view.community = profile_comm.id;
    profile_view.profile_discussion = profile_comm.profile_discussion;

    let disc_id = profile_view.profile_discussion.clone().unwrap();

    let dis_view = get_profile_discussion_view(&ctx_state._db, &ctx, q_params, disc_id).await?;

    profile_view.profile_discussion_view = Some(dis_view);
    let follow_db_service = FollowDbService {
        db: &ctx_state._db,
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
    // let mut dis_view = DiscussionDbService { db: &ctx_state._db, ctx: &ctx }.get_view::<ProfileDiscussionView>(IdentIdName::Id(disc_id.clone())).await?;
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

async fn get_profile_community(db: &Db, ctx: &Ctx, user_id: Thing) -> CtxResult<Community> {
    let comm_db_ser = CommunityDbService { db, ctx };
    comm_db_ser.get_profile_community(user_id).await
}

// posts user is following
async fn get_following_posts(
    State(CtxState { _db, .. }): State<CtxState>,
    ctx: Ctx,
    _q_params: DiscussionParams,
) -> CtxResult<Html<String>> {
    let local_user_db_service = LocalUserDbService {
        db: &_db,
        ctx: &ctx,
    };
    let user_id = local_user_db_service.get_ctx_user_thing().await?;
    let stream_post_ids = PostStreamDbService {
        db: &_db,
        ctx: &ctx,
    }
    .user_posts_stream(user_id)
    .await?;
    let post_list = if stream_post_ids.len() > 0 {
        // TODO resolve view access
        get_entity_list_view::<DiscussionPostView>(
            &_db,
            crate::entity::post_entitiy::TABLE_NAME.to_string(),
            &IdentIdName::Ids(stream_post_ids),
            None,
        )
        .await?
    } else {
        vec![]
    };

    ctx.to_htmx_or_json(FollowingStreamView { post_list })
}

// user chat discussions
async fn get_chats(
    State(CtxState { _db, .. }): State<CtxState>,
    ctx: Ctx,
) -> CtxResult<Html<String>> {
    println!("->> {:<12} - get_chats", "HANDLER");
    let local_user_db_service = LocalUserDbService {
        db: &_db,
        ctx: &ctx,
    };
    let user_id = local_user_db_service.get_ctx_user_thing().await?;
    let comm = get_profile_community(&_db, &ctx, user_id.clone()).await?;
    let discussion_ids = comm.profile_chats.unwrap_or(vec![]);
    /*let discussions = get_entities_by_id::<Discussion>(&_db, discussion_ids).await?;
    let dis = DiscussionDbService{
        db: &_db,
        ctx: &ctx,
    }.get_()*/
    let discussions = get_entity_list_view::<DiscussionView>(
        &_db,
        crate::entity::discussion_entitiy::TABLE_NAME.to_string(),
        &IdentIdName::Ids(discussion_ids),
        None,
    )
    .await?;
    ctx.to_htmx_or_json(ProfileChatList {
        user_id,
        discussions,
    })
}

static ACCEPT_EVENT_NAMES: Lazy<[String; 1]> =
    Lazy::new(|| [UserNotificationEvent::UserChatMessage.to_string()]);

async fn get_chats_sse(
    State(CtxState { _db, .. }): State<CtxState>,
    ctx: Ctx,
) -> CtxResult<Sse<impl FStream<Item = Result<Event, surrealdb::Error>>>> {
    create_user_notifications_sse(
        &_db,
        ctx,
        Vec::from(ACCEPT_EVENT_NAMES.clone()),
        to_sse_event,
    )
    .await?
}

fn to_sse_event(ctx: Ctx, notification: UserNotification) -> CtxResult<Event> {
    let event_ident = notification.event.to_string();
    let event = match notification.event {
        UserNotificationEvent::UserChatMessage { .. } => {
            let post_view = serde_json::from_str::<DiscussionLatestPostView>(&notification.content)
                .map_err(|_| {
                    ctx.to_ctx_error(AppError::Serde {
                        source: notification.content,
                    })
                })?;

            match ctx.to_htmx_or_json(post_view) {
                Ok(response_string) => Event::default().data(response_string.0).event(event_ident),
                Err(err) => {
                    let msg = "ERROR rendering UserNotificationFollowView";
                    println!("{} ERR={}", &msg, err.error);
                    Event::default().data(msg).event("Error".to_string())
                }
            }
        }
        _ => Event::default()
            .data(format!("Event ident {event_ident} recognised"))
            .event("Error".to_string()),
    };

    Ok(event)
}

async fn get_create_chat_discussion(
    State(CtxState { _db, .. }): State<CtxState>,
    ctx: Ctx,
    Path(other_user_id): Path<String>,
) -> CtxResult<Html<String>> {
    println!("->> {:<12} - get get_chat_discussion", "HANDLER");
    let local_user_db_service = LocalUserDbService {
        db: &_db,
        ctx: &ctx,
    };
    let user_id = local_user_db_service.get_ctx_user_thing().await?;
    let other_user_id = get_string_thing(other_user_id)?;
    // TODO limit nr of requests or count them to distinguish bots for user ids
    local_user_db_service
        .exists(IdentIdName::Id(other_user_id.clone()))
        .await?;

    let comm = get_profile_community(&_db, &ctx, user_id.clone()).await?;
    let discussions = comm.profile_chats.clone().unwrap_or(vec![]);
    let comm_db_service = CommunityDbService {
        db: &_db,
        ctx: &ctx,
    };
    let discussion_db_service = DiscussionDbService {
        db: &_db,
        ctx: &ctx,
    };
    let existing = discussion_db_service
        .get_chatroom_with_users(discussions, vec![user_id.clone(), other_user_id.clone()])
        .await?;
    let discussion = match existing {
        None => {
            create_chat_discussion(
                user_id.clone(),
                other_user_id,
                comm,
                comm_db_service,
                discussion_db_service,
            )
            .await?
        }
        Some(disc) => {
            get_discussion_view(
                &_db,
                &ctx,
                disc.id.unwrap(),
                DiscussionParams {
                    topic_id: None,
                    start: None,
                    count: None,
                },
            )
            .await?
        }
    };
    ctx.to_htmx_or_json(ProfileChat {
        discussion,
        user_id,
    })
}

async fn create_chat_discussion<'a>(
    user_id: Thing,
    other_user_id: Thing,
    comm: Community,
    comm_db_service: CommunityDbService<'a>,
    discussion_db_service: DiscussionDbService<'a>,
) -> CtxResult<DiscussionView> {
    let disc = discussion_db_service
        .create_update(Discussion {
            id: None,
            belongs_to: comm.id.unwrap(),
            title: None,
            image_uri: None,
            topics: None,
            chat_room_user_ids: Some(vec![user_id.clone(), other_user_id.clone()]),
            latest_post_id: None,
            r_created: None,
            created_by: user_id.clone(),
        })
        .await?;
    let exists = record_exists(
        comm_db_service.db,
        &CommunityDbService::get_profile_community_id(&user_id),
    )
    .await;
    if exists.is_err() {
        // creates profile community
        get_profile_community(comm_db_service.db, comm_db_service.ctx, user_id.clone()).await?;
    }
    let exists = record_exists(
        comm_db_service.db,
        &CommunityDbService::get_profile_community_id(&other_user_id),
    )
    .await;
    if exists.is_err() {
        // creates profile community
        get_profile_community(
            comm_db_service.db,
            comm_db_service.ctx,
            other_user_id.clone(),
        )
        .await?;
    }

    comm_db_service
        .add_profile_chat_discussion(user_id, disc.id.clone().unwrap())
        .await?;
    comm_db_service
        .add_profile_chat_discussion(other_user_id, disc.id.clone().unwrap())
        .await?;
    let disc = DiscussionView {
        id: disc.id,
        title: disc.title,
        image_uri: disc.image_uri,
        belongs_to: disc.belongs_to,
        chat_room_user_ids: disc.chat_room_user_ids,
        posts: vec![],
        latest_post: None,
        topics: None,
        display_topic: None,
    };
    Ok(disc)
}

async fn create_user_post(
    ctx: Ctx,
    State(ctx_state): State<CtxState>,
    TypedMultipart(input_value): TypedMultipart<PostInput>,
) -> CtxResult<Response> {
    let user_id = LocalUserDbService {
        db: &ctx_state._db,
        ctx: &ctx,
    }
    .get_ctx_user_thing()
    .await?;
    let profile_comm = get_profile_community(&ctx_state._db, &ctx, user_id).await?;

    create_post_entity_route(
        ctx,
        Path(profile_comm.profile_discussion.unwrap().to_raw()),
        State(ctx_state),
        TypedMultipart(input_value),
    )
    .await
}

async fn get_user_posts(
    State(ctx_state): State<CtxState>,
    ctx: Ctx,
    Path(username): Path<String>,
    q_params: DiscussionParams,
) -> CtxResult<Html<String>> {
    let local_user_db_service = LocalUserDbService {
        db: &ctx_state._db,
        ctx: &ctx,
    };
    let user = local_user_db_service
        .get(UsernameIdent(username).into())
        .await?;
    let profile_comm =
        get_profile_community(&ctx_state._db, &ctx, user.id.expect("user id").clone()).await?;
    let profile_disc_view = get_profile_discussion_view(
        &ctx_state._db,
        &ctx,
        q_params,
        profile_comm.profile_discussion.expect("profile discussion"),
    )
    .await?;
    ctx.to_htmx_or_json(profile_disc_view)
}

async fn search_users(
    ctx: Ctx,
    State(ctx_state): State<CtxState>,
    JsonOrFormValidated(form_value): JsonOrFormValidated<SearchInput>,
) -> CtxResult<Html<String>> {
    let local_user_db_service = LocalUserDbService {
        db: &ctx_state._db,
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
