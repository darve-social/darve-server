use crate::{
    entities::{community::post_entity::PostDbService, user_auth::local_user_entity::LocalUser},
    middleware::{ctx::Ctx, db, error::CtxResult, utils::string_utils::get_string_thing},
};

use surrealdb::sql::Thing;

pub struct PostService<'a> {
    pub db: &'a db::Db,
    pub ctx: &'a Ctx,
}

impl<'a> PostService<'a> {
    pub async fn like(&self, post_id: &Thing, user: &LocalUser) -> CtxResult<u32> {
        let user_thing = user.id.clone().expect("User id invalid");
        let post_service = PostDbService {
            db: &self.db,
            ctx: &self.ctx,
        };

        let likes_count = post_service
            .like(user_thing.clone(), post_id.clone())
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
