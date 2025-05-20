use crate::{
    entities::{
        community::post_entity::PostDbService,
        user_auth::{
            follow_entity::FollowDbService,
            local_user_entity::LocalUser,
            user_notification_entity::{UserNotificationDbService, UserNotificationEvent},
        },
    },
    middleware::{
        ctx::Ctx,
        db,
        error::CtxResult,
        utils::{db_utils::IdentIdName, string_utils::get_string_thing},
    },
};

use surrealdb::sql::Thing;
pub struct PostService<'a> {
    pub db: &'a db::Db,
    pub ctx: &'a Ctx,
}

impl<'a> PostService<'a> {
    pub async fn like(&self, post_id: String, user: &LocalUser) -> CtxResult<()> {
        let user_thing = user.id.clone().expect("User id invalid");
        let post_thing = get_string_thing(post_id)?;
        let post_service = PostDbService {
            db: &self.db,
            ctx: &self.ctx,
        };

        let post = post_service
            .get(IdentIdName::Id(post_thing.clone()))
            .await?;

        post_service
            .like(user_thing.clone(), post_thing.clone())
            .await?;

        let user_ids = FollowDbService {
            db: &self.db,
            ctx: &self.ctx,
        }
        .user_follower_ids(user_thing.clone())
        .await
        .unwrap_or_default()
        .into_iter()
        .filter(|id| *id != user_thing)
        .collect::<Vec<Thing>>();

        if user_ids.len() > 0 {
            UserNotificationDbService {
                db: &self.db,
                ctx: &self.ctx,
            }
            .notify_users(
                user_ids,
                &&&UserNotificationEvent::UserLikePost {
                    user_id: user_thing,
                    post_id: post_thing,
                },
                &format!("`{}` likes the `{}` post", user.username, post.title),
            )
            .await?;
        }
        Ok(())
    }

    pub async fn unlike(&self, post_id: String, user: &LocalUser) -> CtxResult<()> {
        let post_thing = get_string_thing(post_id)?;
        let user_thing = user.id.clone().expect("User id invalid");

        let post_service = PostDbService {
            db: &self.db,
            ctx: &self.ctx,
        };

        let post = post_service
            .get(IdentIdName::Id(post_thing.clone()))
            .await?;

        post_service
            .unlike(user_thing.clone(), post_thing.clone())
            .await?;

        PostDbService {
            db: &self.db,
            ctx: &self.ctx,
        }
        .unlike(user_thing.clone(), post_thing.clone())
        .await?;

        let user_ids = FollowDbService {
            db: &self.db,
            ctx: &self.ctx,
        }
        .user_follower_ids(user_thing.clone())
        .await
        .unwrap_or_default()
        .into_iter()
        .filter(|id| *id != user_thing)
        .collect::<Vec<Thing>>();

        if user_ids.len() > 0 {
            UserNotificationDbService {
                db: &self.db,
                ctx: &self.ctx,
            }
            .notify_users(
                user_ids,
                &&&UserNotificationEvent::UserUnLikePost {
                    user_id: user_thing,
                    post_id: post_thing,
                },
                &format!("`{}` dislikes the `{}` post", user.username, post.title),
            )
            .await?;
        }
        Ok(())
    }
}
