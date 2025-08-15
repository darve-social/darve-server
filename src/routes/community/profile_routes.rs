use std::sync::Arc;

use askama_axum::Template;
use axum::extract::{Path, Query, State};
use axum::routing::get;
use axum::{Json, Router};
use axum_typed_multipart::TryFromMultipart;
use serde::{Deserialize, Serialize};
use surrealdb::sql::Thing;

use community::post_entity;
use community_entity::CommunityDbService;
use follow_entity::FollowDbService;
use local_user_entity::LocalUserDbService;
use middleware::ctx::Ctx;

use crate::entities::community::discussion_entity::DiscussionDbService;
use crate::entities::community::{self, community_entity};
use crate::entities::user_auth::{follow_entity, local_user_entity};
use crate::services::post_service::PostView;
use crate::{middleware, utils};
use middleware::error::CtxResult;
use middleware::mw_ctx::CtxState;
use middleware::utils::db_utils::{IdentIdName, ViewFieldSelector};
use middleware::utils::extractor_utils::DiscussionParams;
use middleware::utils::string_utils::get_string_thing;
use post_entity::PostDbService;
use utils::askama_filter_util::filters;

pub fn routes() -> Router<Arc<CtxState>> {
    Router::new().route("/u/:username_or_id", get(display_profile))
}

#[derive(Template, TryFromMultipart)]
#[template(path = "nera2/profile_settings_form.html")]
pub struct ProfileSettingsForm {
    pub username: String,
    pub full_name: String,
    pub email: String,
    pub image_url: String,
}

#[derive(Template, Serialize, Deserialize, Debug)]
#[template(path = "nera2/profile_page.html")]
pub struct ProfilePage {
    theme_name: String,
    window_title: String,
    nav_top_title: String,
    header_title: String,
    footer_text: String,
    pub profile_view: Option<ProfileView>,
}

#[derive(Template, Serialize, Deserialize, Debug)]
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
    pub is_otp_enabled: bool,
    pub social_links: Option<Vec<String>>,
    pub community: Option<Thing>,
    pub profile_discussion: Option<Thing>,
    pub idea_discussion: Option<Thing>,
    #[serde(default)]
    pub followers_nr: i64,
    #[serde(default)]
    pub following_nr: i64,
    #[serde(default)]
    pub posts: Vec<PostView>,
}

impl ViewFieldSelector for ProfileView {
    fn get_select_query_fields() -> String {
        "*, id as user_id".to_string()
    }
}

async fn display_profile(
    State(ctx_state): State<Arc<CtxState>>,
    ctx: Ctx,
    Path(username_or_id): Path<String>,
    Query(q_params): Query<DiscussionParams>,
) -> CtxResult<Json<ProfileView>> {
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

    profile_view.community = Some(CommunityDbService::get_profile_community_id(
        &profile_view.user_id,
    ));
    profile_view.profile_discussion = Some(DiscussionDbService::get_profile_discussion_id(
        &profile_view.user_id,
    ));

    profile_view.idea_discussion = Some(DiscussionDbService::get_idea_discussion_id(
        &profile_view.user_id,
    ));

    let disc_id = profile_view.profile_discussion.clone().unwrap();

    profile_view.posts = PostDbService {
        db: &ctx_state.db.client,
        ctx: &ctx,
    }
    .get_by_discussion_desc_view::<PostView>(disc_id.clone(), q_params.clone())
    .await?;

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

    Ok(Json(profile_view))
}
