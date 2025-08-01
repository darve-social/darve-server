use std::sync::Arc;
use std::vec;

use crate::entities::community::discussion_entity::DiscussionDbService;
use crate::entities::community::post_entity::PostDbService;
use crate::entities::community::post_stream_entity::PostStreamDbService;
use crate::entities::user_auth::{self};
use crate::middleware;
use crate::middleware::auth_with_login_access::AuthWithLoginAccess;
use crate::middleware::utils::db_utils::RecordWithId;
use crate::middleware::utils::extractor_utils::DiscussionParams;
use crate::middleware::utils::string_utils::get_str_thing;
use crate::services::notification_service::NotificationService;
use askama_axum::Template;
use axum::extract::{Path, State};
use axum::routing::{delete, get, post};
use axum::{Json, Router};
use follow_entity::FollowDbService;
use local_user_entity::{LocalUser, LocalUserDbService};
use middleware::ctx::Ctx;
use middleware::error::CtxResult;
use middleware::mw_ctx::CtxState;
use middleware::utils::db_utils::IdentIdName;
use middleware::utils::string_utils::get_string_thing;
use serde::{Deserialize, Serialize};
use surrealdb::sql::Thing;
use user_auth::{follow_entity, local_user_entity};

pub fn routes() -> Router<Arc<CtxState>> {
    Router::new()
        .route("/api/users/:user_id/followers", get(get_followers))
        .route("/api/users/:user_id/following", get(get_following))
        .route(
            "/api/users/:user_id/followers/count",
            get(get_followers_count),
        )
        .route(
            "/api/users/:user_id/following/count",
            get(get_following_count),
        )
        .route("/api/followers/:follow_user_id", post(follow_user))
        .route("/api/followers/:follow_user_id", delete(unfollow_user))
        .route("/api/followers/:follow_user_id", get(is_following_user))
}

#[derive(Template, Serialize, Deserialize, Debug)]
#[template(path = "nera2/follow-user.html")]
pub struct UserItemView {
    pub id: Thing,
    pub username: String,
    pub name: String,
    pub image_url: String,
}

impl From<LocalUser> for UserItemView {
    fn from(value: LocalUser) -> Self {
        UserItemView {
            id: value.id.unwrap(),
            username: value.username.clone(),
            name: value.full_name.clone().unwrap_or_default(),
            image_url: value.image_uri.clone().unwrap_or_default(),
        }
    }
}

async fn get_followers_count(
    State(ctx_state): State<Arc<CtxState>>,
    ctx: Ctx,
    Path(user_id): Path<String>,
) -> CtxResult<Json<u32>> {
    let user_id = get_str_thing(&user_id)?;
    let count = FollowDbService {
        db: &ctx_state.db.client,
        ctx: &ctx,
    }
    .user_followers_number(user_id)
    .await?;
    Ok(Json(count as u32))
}

async fn get_following_count(
    State(ctx_state): State<Arc<CtxState>>,
    ctx: Ctx,
    Path(user_id): Path<String>,
) -> CtxResult<Json<u32>> {
    let user_id = get_str_thing(&user_id)?;
    let count = FollowDbService {
        db: &ctx_state.db.client,
        ctx: &ctx,
    }
    .user_following_number(user_id)
    .await?;

    Ok(Json(count as u32))
}

async fn get_followers(
    State(ctx_state): State<Arc<CtxState>>,
    ctx: Ctx,
    Path(user_id): Path<String>,
) -> CtxResult<Json<Vec<UserItemView>>> {
    let user_id = get_string_thing(user_id.clone())?;
    let followers: Vec<UserItemView> = FollowDbService {
        db: &ctx_state.db.client,
        ctx: &ctx,
    }
    .user_followers(user_id)
    .await?
    .into_iter()
    .map(UserItemView::from)
    .collect();

    Ok(Json(followers))
}

async fn get_following(
    State(ctx_state): State<Arc<CtxState>>,
    ctx: Ctx,
    Path(user_id): Path<String>,
) -> CtxResult<Json<Vec<UserItemView>>> {
    let user_id = get_string_thing(user_id.clone())?;
    let following = FollowDbService {
        db: &ctx_state.db.client,
        ctx: &ctx,
    }
    .user_following(user_id)
    .await?
    .into_iter()
    .map(UserItemView::from)
    .collect();
    Ok(Json(following))
}

async fn follow_user(
    State(ctx_state): State<Arc<CtxState>>,
    auth_data: AuthWithLoginAccess,
    Path(follow_user_id): Path<String>,
) -> CtxResult<()> {
    let local_user_db_service = LocalUserDbService {
        db: &ctx_state.db.client,
        ctx: &auth_data.ctx,
    };
    let from_user = local_user_db_service.get_ctx_user().await?;
    let follow = get_string_thing(follow_user_id.clone())?;

    let follows_username = local_user_db_service
        .get_username(IdentIdName::Id(follow.clone()))
        .await?;

    let follow_db_service = FollowDbService {
        db: &ctx_state.db.client,
        ctx: &auth_data.ctx,
    };

    let success = follow_db_service
        .create_follow(from_user.id.clone().unwrap(), follow.clone())
        .await?;

    if success {
        let mut follower_ids = follow_db_service
            .user_follower_ids(from_user.id.clone().unwrap())
            .await?;
        follower_ids.push(follow.clone());

        let n_service = NotificationService::new(
            &ctx_state.db.client,
            &auth_data.ctx,
            &ctx_state.event_sender,
            &ctx_state.db.user_notifications,
        );
        n_service
            .on_follow(&from_user, follows_username, follower_ids)
            .await?;

        let _ = add_latest_posts(&from_user.id.unwrap(), &follow, &ctx_state, &auth_data.ctx).await;
    }

    Ok(())
}

async fn unfollow_user(
    State(ctx_state): State<Arc<CtxState>>,
    auth_data: AuthWithLoginAccess,
    Path(follow_user_id): Path<String>,
) -> CtxResult<()> {
    let user_id = LocalUserDbService {
        db: &ctx_state.db.client,
        ctx: &auth_data.ctx,
    }
    .get_ctx_user_thing()
    .await?;
    let follow = get_string_thing(follow_user_id.clone())?;
    let _ = FollowDbService {
        db: &ctx_state.db.client,
        ctx: &auth_data.ctx,
    }
    .remove_follow(user_id, follow)
    .await?;
    Ok(())
}

async fn is_following_user(
    State(ctx_state): State<Arc<CtxState>>,
    auth_data: AuthWithLoginAccess,
    Path(follow_user_id): Path<String>,
) -> CtxResult<()> {
    let user_id = LocalUserDbService {
        db: &ctx_state.db.client,
        ctx: &auth_data.ctx,
    }
    .get_ctx_user_thing()
    .await?;
    let follows_user = get_string_thing(follow_user_id.clone())?;
    let _ = FollowDbService {
        db: &ctx_state.db.client,
        ctx: &auth_data.ctx,
    }
    .is_following(user_id, follows_user.clone())
    .await?;
    Ok(())
}

async fn add_latest_posts(
    ctx_user_id: &Thing,
    follow_user_id: &Thing,
    ctx_state: &CtxState,
    ctx: &Ctx,
) {
    // let follow_profile_comm = match (CommunityDbService {
    //     ctx: &ctx,
    //     db: &ctx_state.db.client,
    // }
    // .get_profile_community(follow_user_id.to_owned())
    // .await)
    // {
    //     Ok(res) => res,
    //     Err(err) => {
    //         println!("get_profile_community error / err={err:?}");
    //         return;
    //     }
    // };
    let follow_profile_discussion_id =
        DiscussionDbService::get_profile_discussion_id(&follow_user_id.to_owned());

    let post_db_service = PostDbService {
        ctx: &ctx,
        db: &ctx_state.db.client,
    };

    let latest_posts = match post_db_service
        .get_by_discussion_desc_view::<RecordWithId>(
            follow_profile_discussion_id,
            DiscussionParams {
                topic_id: None,
                start: Some(0),
                count: Some(3),
            },
        )
        .await
    {
        Ok(res) => res,
        Err(err) => {
            println!(" err getting latest posts / err={err:?}");
            return;
        }
    };

    let stream_db_service = PostStreamDbService {
        ctx: &ctx,
        db: &ctx_state.db.client,
    };
    for post in latest_posts {
        if let Err(err) = stream_db_service
            .add_to_users_stream(vec![ctx_user_id.clone()], &post.id)
            .await
        {
            println!(" error adding to stream / err{err:?}");
            continue;
        };
    }
}
