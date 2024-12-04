use std::io::Write;
use std::str::FromStr;

use askama_axum::axum_core::response::IntoResponse;
use askama_axum::Template;
use axum::extract::{Path, State};
use axum::response::Html;
use axum::routing::{delete, get, post};
use axum::Router;
use futures::TryFutureExt;
use serde::{Deserialize, Serialize};

use crate::entity::follow_entitiy::FollowDbService;
use crate::entity::local_user_entity::{LocalUser, LocalUserDbService};
use crate::entity::user_notification_entitiy::{UserNotification, UserNotificationDbService, UserNotificationEvent};
use sb_middleware::ctx::Ctx;
use sb_middleware::error::CtxResult;
use sb_middleware::mw_ctx::CtxState;
use sb_middleware::utils::db_utils::IdentIdName;
use sb_middleware::utils::request_utils::CreatedResponse;
use sb_middleware::utils::string_utils::get_string_thing;

pub fn routes(state: CtxState) -> Router {
    Router::new()
        .route("/api/user/:user_id/followers", get(get_followers))
        .route("/api/user/:user_id/following", get(get_following))
        .route("/api/follow/:follow_user_id", post(follow_user))
        .route("/api/follow/:follow_user_id", delete(unfollow_user))
        .route("/api/user/follows/:follows_user_id", get(is_following_user))
        .with_state(state)
}

#[derive(Template, Serialize, Deserialize, Debug)]
#[template(path = "nera2/follow-user-list.html")]
pub struct FollowUserList {
    pub list: Vec<FollowUser>,
}


#[derive(Template, Serialize, Deserialize, Debug)]
#[template(path = "nera2/follow-user.html")]
pub struct FollowUser {
    pub username: String,
    pub name: String,
    pub image_url: String,
}

impl From<LocalUser> for FollowUser {
    fn from(value: LocalUser) -> Self {
        FollowUser {
            username: value.username.clone(),
            name: value.full_name.clone().unwrap_or_default(),
            image_url: value.image_uri.clone().unwrap_or_default(),
        }
    }
}

async fn get_followers(
    State(ctx_state): State<CtxState>,
    ctx: Ctx,
    Path(user_id): Path<String>,
) -> CtxResult<Html<String>> {
    let user_id = get_string_thing(user_id.clone())?;
    let followers: Vec<FollowUser> = FollowDbService { db: &ctx_state._db, ctx: &ctx }.user_followers(user_id).await?
        .into_iter().map(FollowUser::from).collect();
    ctx.to_htmx_or_json(FollowUserList { list: followers })
}

async fn get_following(
    State(ctx_state): State<CtxState>,
    ctx: Ctx,
    Path(user_id): Path<String>,
) -> CtxResult<Html<String>> {
    let user_id = get_string_thing(user_id.clone())?;
    let following = FollowDbService { db: &ctx_state._db, ctx: &ctx }.user_following(user_id).await?
        .into_iter().map(FollowUser::from).collect();
    ctx.to_htmx_or_json(FollowUserList { list: following })
}

async fn follow_user(
    State(ctx_state): State<CtxState>,
    ctx: Ctx,
    Path(follow_user_id): Path<String>,
) -> CtxResult<Html<String>> {
    let local_user_db_service = LocalUserDbService { db: &ctx_state._db, ctx: &ctx };
    let from_user = local_user_db_service.get_ctx_user().await?;
    let follow = get_string_thing(follow_user_id.clone())?;
    let success = FollowDbService { db: &ctx_state._db, ctx: &ctx }.create_follow(from_user.id.clone().unwrap(), follow.clone()).await?;
    if success {
        let follows_username = local_user_db_service.get_username(IdentIdName::Id(follow.clone())).await?;
        let mut follower_ids = FollowDbService{ db: &ctx_state._db, ctx: &ctx }.user_follower_ids(from_user.id.clone().unwrap()).await?;
        follower_ids.push(follow);
        UserNotificationDbService{ db: &ctx_state._db, ctx: &ctx }
            .notify_users(follower_ids, &UserNotificationEvent::UserFollowAdded {username: from_user.username, follows_username  },"").await?;
    }
    ctx.to_htmx_or_json(CreatedResponse { id: follow_user_id, success, uri: None })
}

async fn unfollow_user(
    State(ctx_state): State<CtxState>,
    ctx: Ctx,
    Path(unfollow_user_id): Path<String>,
) -> CtxResult<Html<String>> {
    let user_id = LocalUserDbService { db: &ctx_state._db, ctx: &ctx }.get_ctx_user_thing().await?;
    let follow = get_string_thing(unfollow_user_id.clone())?;
    let success = FollowDbService { db: &ctx_state._db, ctx: &ctx }.remove_follow(user_id, follow).await?;
    ctx.to_htmx_or_json(CreatedResponse { id: unfollow_user_id, success, uri: None })
}

async fn is_following_user(
    State(ctx_state): State<CtxState>,
    ctx: Ctx,
    Path(following_user_id): Path<String>,
) -> CtxResult<Html<String>> {
    let user_id = LocalUserDbService { db: &ctx_state._db, ctx: &ctx }.get_ctx_user_thing().await?;
    let follows_user = get_string_thing(following_user_id.clone())?;
    let success = FollowDbService { db: &ctx_state._db, ctx: &ctx }.is_following(user_id, follows_user.clone()).await?;
    ctx.to_htmx_or_json(CreatedResponse { id: follows_user.to_raw(), success, uri: None })
}
