use crate::{
    entities::{
        community::post_entity::PostDbService,
        user_auth::{
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

pub struct PostService<'a> {
    pub db: &'a db::Db,
    pub ctx: &'a Ctx,
}

impl<'a> PostService<'a> {
    pub async fn like(&self, post_id: String, user: &LocalUser) -> CtxResult<u32> {
        let user_thing = user.id.clone().expect("User id invalid");
        let post_thing = get_string_thing(post_id)?;
        let post_service = PostDbService {
            db: &self.db,
            ctx: &self.ctx,
        };

        let likes_count = post_service
            .like(user_thing.clone(), post_thing.clone())
            .await?;

        UserNotificationDbService {
            db: &self.db,
            ctx: &self.ctx,
        }
        .notify_users(
            vec![user_thing.clone()],
            &UserNotificationEvent::UserLikePost {
                user_id: user_thing,
                post_id: post_thing,
            },
            "asdasdas",
        )
        .await?;

        Ok(likes_count)
    }

    pub async fn unlike(&self, post_id: String, user: &LocalUser) -> CtxResult<u32> {
        let post_thing = get_string_thing(post_id)?;
        let user_thing = user.id.clone().expect("User id invalid");

        let post_service = PostDbService {
            db: &self.db,
            ctx: &self.ctx,
        };

        let likes_count = post_service
            .unlike(user_thing.clone(), post_thing.clone())
            .await?;

        Ok(likes_count)
    }
}
