use std::sync::Arc;

use askama_axum::Template;
use axum::extract::{DefaultBodyLimit, Path, Query, State};
use axum::response::Html;
use axum::routing::get;
use axum::Router;
use axum_typed_multipart::TryFromMultipart;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use surrealdb::sql::Thing;

use community::post_entity;
use community_entity::CommunityDbService;
use discussion_routes::DiscussionPostView;
use follow_entity::FollowDbService;
use local_user_entity::LocalUserDbService;
use middleware::ctx::Ctx;

use middleware::error::CtxResult;
use middleware::mw_ctx::CtxState;
use middleware::utils::db_utils::{IdentIdName, ViewFieldSelector};
use middleware::utils::extractor_utils::DiscussionParams;
use middleware::utils::string_utils::get_string_thing;
use post_entity::PostDbService;
use utils::askama_filter_util::filters;
use utils::template_utils::ProfileFormPage;

use super::discussion_routes;
use crate::database::client::Db;
use crate::entities::community::{self, community_entity};
use crate::entities::user_auth::{follow_entity, local_user_entity};
use crate::routes::community::discussion_routes::DiscussionView;
use crate::{middleware, utils};

pub fn routes(upload_max_size_mb: u64) -> Router<Arc<CtxState>> {
    let max_bytes_val = (1024 * 1024 * upload_max_size_mb) as usize;
    Router::new()
        .route("/u/:username_or_id", get(display_profile))
        .route("/accounts/edit", get(profile_form))
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
    pub email: Option<String>,
    pub birth_date: Option<String>,
    pub phone: Option<String>,
    pub image_uri: Option<String>,
    #[serde(default)]
    pub is_otp_enbaled: bool,
    pub social_links: Option<Vec<String>>,
    pub community: Option<Thing>,
    pub default_discussion: Option<Thing>,
    #[serde(default)]
    pub followers_nr: i64,
    #[serde(default)]
    pub following_nr: i64,
    pub default_discussion_view: Option<ProfileDiscussionView>,
}

impl ViewFieldSelector for ProfileView {
    fn get_select_query_fields() -> String {
        "*, id as user_id".to_string()
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
    pub title: String,
    pub content: Option<String>,
    pub media_links: Option<Vec<String>>,
    pub created_at: DateTime<Utc>,
    pub replies_nr: i64,
    pub likes_nr: i64,
}

impl ViewFieldSelector for ProfilePostView {
    // post fields selct qry for view
    fn get_select_query_fields() -> String {
        "id, created_by.username as username,  title, content, media_links, created_at, belongs_to.id as belongs_to_id, replies_nr, likes_nr".to_string()
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

    let comm_db_ser = CommunityDbService {
        db: &ctx_state.db.client,
        ctx: &ctx,
    };
    let profile_comm = comm_db_ser
        .get_profile_community(profile_view.user_id.clone())
        .await?;
    profile_view.community = profile_comm.id;
    profile_view.default_discussion = profile_comm.default_discussion;

    let disc_id = profile_view.default_discussion.clone().unwrap();

    let dis_view =
        get_profile_discussion_view(&ctx_state.db.client, &ctx, q_params, disc_id).await?;

    profile_view.default_discussion_view = Some(dis_view);
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
