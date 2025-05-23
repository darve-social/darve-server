use askama_axum::Template;
use axum::extract::{Path, State};
use axum::response::Html;
use axum::routing::{delete, get, post};
use axum::Router;
use follow_entity::FollowDbService;
use local_user_entity::{LocalUser, LocalUserDbService};
use middleware::ctx::Ctx;
use middleware::error::CtxResult;
use middleware::mw_ctx::CtxState;
use middleware::utils::db_utils::IdentIdName;
use middleware::utils::request_utils::CreatedResponse;
use middleware::utils::string_utils::get_string_thing;
use serde::{Deserialize, Serialize};
use surrealdb::sql::Thing;
use user_auth::{follow_entity, local_user_entity};
use user_notification_entity::{UserNotificationDbService, UserNotificationEvent};

use crate::entities::community::community_entity::CommunityDbService;
use crate::entities::community::discussion_entity::DiscussionDbService;
use crate::entities::community::post_entity::PostDbService;
use crate::entities::community::post_stream_entity::PostStreamDbService;
use crate::entities::user_auth::{self, user_notification_entity};
use crate::middleware;
use crate::middleware::utils::db_utils::RecordWithId;
use crate::middleware::utils::extractor_utils::DiscussionParams;

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
pub struct UserListView {
    pub items: Vec<UserItemView>,
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

async fn get_followers(
    State(ctx_state): State<CtxState>,
    ctx: Ctx,
    Path(user_id): Path<String>,
) -> CtxResult<Html<String>> {
    let user_id = get_string_thing(user_id.clone())?;
    let followers: Vec<UserItemView> = FollowDbService {
        db: &ctx_state._db,
        ctx: &ctx,
    }
    .user_followers(user_id)
    .await?
    .into_iter()
    .map(UserItemView::from)
    .collect();
    ctx.to_htmx_or_json(UserListView { items: followers })
}

async fn get_following(
    State(ctx_state): State<CtxState>,
    ctx: Ctx,
    Path(user_id): Path<String>,
) -> CtxResult<Html<String>> {
    let user_id = get_string_thing(user_id.clone())?;
    let following = FollowDbService {
        db: &ctx_state._db,
        ctx: &ctx,
    }
    .user_following(user_id)
    .await?
    .into_iter()
    .map(UserItemView::from)
    .collect();
    ctx.to_htmx_or_json(UserListView { items: following })
}

async fn follow_user(
    State(ctx_state): State<CtxState>,
    ctx: Ctx,
    Path(follow_user_id): Path<String>,
) -> CtxResult<Html<String>> {
    let local_user_db_service = LocalUserDbService {
        db: &ctx_state._db,
        ctx: &ctx,
    };
    let from_user = local_user_db_service.get_ctx_user().await?;
    let follow = get_string_thing(follow_user_id.clone())?;

    let follows_username = local_user_db_service
        .get_username(IdentIdName::Id(follow.clone()))
        .await?;

    let follow_db_service = FollowDbService {
        db: &ctx_state._db,
        ctx: &ctx,
    };

    let success = follow_db_service
        .create_follow(from_user.id.clone().unwrap(), follow.clone())
        .await?;

    if success {
        let mut follower_ids = follow_db_service
            .user_follower_ids(from_user.id.clone().unwrap())
            .await?;
        follower_ids.push(follow.clone());
        UserNotificationDbService {
            db: &ctx_state._db,
            ctx: &ctx,
        }
        .notify_users(
            follower_ids,
            &UserNotificationEvent::UserFollowAdded {
                username: from_user.username,
                follows_username,
            },
            "",
        )
        .await?;

        let _ = add_latest_posts(&from_user.id.unwrap(), &follow, &ctx_state, &ctx).await;
    }

    ctx.to_htmx_or_json(CreatedResponse {
        id: follow_user_id,
        success,
        uri: None,
    })
}

async fn unfollow_user(
    State(ctx_state): State<CtxState>,
    ctx: Ctx,
    Path(unfollow_user_id): Path<String>,
) -> CtxResult<Html<String>> {
    let user_id = LocalUserDbService {
        db: &ctx_state._db,
        ctx: &ctx,
    }
    .get_ctx_user_thing()
    .await?;
    let follow = get_string_thing(unfollow_user_id.clone())?;
    let success = FollowDbService {
        db: &ctx_state._db,
        ctx: &ctx,
    }
    .remove_follow(user_id, follow)
    .await?;
    ctx.to_htmx_or_json(CreatedResponse {
        id: unfollow_user_id,
        success,
        uri: None,
    })
}

async fn is_following_user(
    State(ctx_state): State<CtxState>,
    ctx: Ctx,
    Path(following_user_id): Path<String>,
) -> CtxResult<Html<String>> {
    let user_id = LocalUserDbService {
        db: &ctx_state._db,
        ctx: &ctx,
    }
    .get_ctx_user_thing()
    .await?;
    let follows_user = get_string_thing(following_user_id.clone())?;
    let success = FollowDbService {
        db: &ctx_state._db,
        ctx: &ctx,
    }
    .is_following(user_id, follows_user.clone())
    .await?;
    ctx.to_htmx_or_json(CreatedResponse {
        id: follows_user.to_raw(),
        success,
        uri: None,
    })
}

async fn add_latest_posts(
    ctx_user_id: &Thing,
    follow_user_id: &Thing,
    ctx_state: &CtxState,
    ctx: &Ctx,
) {
    // let follow_profile_comm = match (CommunityDbService {
    //     ctx: &ctx,
    //     db: &ctx_state._db,
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
    let follow_profile_discussion_id = DiscussionDbService::get_profile_discussion_id(&follow_user_id.to_owned());

    let post_db_service = PostDbService {
        ctx: &ctx,
        db: &ctx_state._db,
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
        db: &ctx_state._db,
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
