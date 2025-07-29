use crate::entities::user_auth::access_right_entity::AccessRightDbService;
use crate::middleware::utils::db_utils::{record_exist_all, IdentIdName};
use crate::middleware::utils::string_utils::get_str_thing;
use crate::{
    entities::{
        community::discussion_entity::{Discussion, DiscussionDbService},
        user_auth::local_user_entity::LocalUserDbService,
    },
    middleware::{
        ctx::Ctx,
        error::{AppError, AppResult, CtxResult},
        mw_ctx::CtxState,
    },
};
use serde::{Deserialize, Serialize};
use surrealdb::sql::Thing;
use validator::Validate;

#[derive(Deserialize, Serialize, Validate)]
pub struct CreateDiscussion {
    pub community_id: String,
    #[validate(length(min = 5, message = "Min 5 characters"))]
    pub title: String,
    pub image_uri: Option<String>,
    pub chat_user_ids: Option<Vec<String>>,
    pub private_discussion_users_final: bool,
}

#[derive(Debug, Deserialize, Serialize, Validate)]
pub struct UpdateDiscussion {
    pub title: Option<String>,
}

pub struct DiscussionService<'a> {
    ctx: &'a Ctx,
    user_repository: LocalUserDbService<'a>,
    discussion_repository: DiscussionDbService<'a>,
    access_right_repository: AccessRightDbService<'a>,
}

impl<'a> DiscussionService<'a> {
    pub fn new(state: &'a CtxState, ctx: &'a Ctx) -> Self {
        Self {
            ctx,
            user_repository: LocalUserDbService {
                db: &state.db.client,
                ctx: &ctx,
            },
            discussion_repository: DiscussionDbService {
                db: &state.db.client,
                ctx: &ctx,
            },
            access_right_repository: AccessRightDbService {
                db: &state.db.client,
                ctx: &ctx,
            },
        }
    }

    pub async fn delete(&self, user_id: &str, disc_id: &str) -> AppResult<()> {
        let user_id: Thing = get_str_thing(user_id)?;
        self.access_right_repository
            .has_owner_access(&user_id, disc_id)
            .await?;
        self.discussion_repository.delete(disc_id).await?;
        Ok(())
    }

    pub async fn update(
        &self,
        user_id: &str,
        disc_id: &str,
        data: UpdateDiscussion,
    ) -> AppResult<()> {
        let user_id: Thing = get_str_thing(user_id)?;
        let disc_id = self
            .access_right_repository
            .has_owner_access(&user_id, disc_id)
            .await?;

        let mut disc = self
            .discussion_repository
            .get(IdentIdName::Id(disc_id))
            .await?;
        disc.title = data.title;
        self.discussion_repository.create_update(disc).await?;

        Ok(())
    }

    pub async fn add_chat_users(
        &self,
        user_id: &str,
        disc_id: &str,
        new_user_ids: Vec<String>,
    ) -> CtxResult<Vec<Thing>> {
        if new_user_ids.is_empty() {
            return Err(AppError::Generic {
                description: "no users present".to_string(),
            }
            .into());
        }

        let user_id: Thing = get_str_thing(user_id)?;
        let disc_id = self
            .access_right_repository
            .has_owner_access(&user_id, disc_id)
            .await?;

        let mut disc = self
            .discussion_repository
            .get(IdentIdName::Id(disc_id))
            .await?;

        if disc.private_discussion_users_final {
            return Err(AppError::Generic {
                description: "Forbidden".to_string(),
            }
            .into());
        };

        let user_ids = record_exist_all(self.user_repository.db, new_user_ids).await?;

        let mut chat_users = disc.private_discussion_user_ids.unwrap_or(vec![]);

        user_ids.into_iter().for_each(|id| {
            if !chat_users.contains(&id) {
                chat_users.push(id);
            }
        });

        disc.private_discussion_user_ids = Some(chat_users);
        let chat_users = self
            .discussion_repository
            .create_update(disc)
            .await?
            .private_discussion_user_ids
            .expect("users are set");

        Ok(chat_users)
    }

    pub async fn remove_chat_users(
        &self,
        disc_id: &str,
        remove_user_ids: Vec<String>,
    ) -> CtxResult<()> {
        if remove_user_ids.is_empty() {
            return Err(self.ctx.to_ctx_error(AppError::Generic {
                description: "no users present".to_string(),
            }));
        }
        let user_id_str = self.ctx.user_id()?;
        let user_id = get_str_thing(&user_id_str)?;

        let disc_id = self
            .access_right_repository
            .has_owner_access(&user_id, disc_id)
            .await?;

        if remove_user_ids.contains(&user_id_str) {
            return Err(self.ctx.to_ctx_error(AppError::Generic {
                description: "Owner of the discussion can not remove yourself".to_string(),
            }));
        };

        let mut disc = self
            .discussion_repository
            .get(IdentIdName::Id(disc_id))
            .await?;

        if disc.private_discussion_users_final {
            return Err(AppError::Generic {
                description: "Forbidden".to_string(),
            }
            .into());
        };

        if disc.private_discussion_user_ids.is_none()
            || disc
                .private_discussion_user_ids
                .as_ref()
                .unwrap()
                .is_empty()
        {
            return Err(AppError::Generic {
                description: "Forbidden".to_string(),
            }
            .into());
        };

        let mut remove_things = Vec::with_capacity(remove_user_ids.len());
        for id in remove_user_ids {
            match get_str_thing(&id) {
                Ok(v) => remove_things.push(v),
                Err(_) => {
                    return Err(AppError::Generic {
                        description: "Invalid user id".to_string(),
                    }
                    .into())
                }
            };
        }

        let private_discussion_user_ids = disc
            .private_discussion_user_ids
            .unwrap()
            .into_iter()
            .filter(|id| !remove_things.contains(id))
            .collect::<Vec<Thing>>();

        disc.private_discussion_user_ids = Some(private_discussion_user_ids);
        self.discussion_repository.create_update(disc).await?;

        Ok(())
    }

    pub async fn get_by_chat_user(&self, user_id: &str) -> CtxResult<Vec<Discussion>> {
        self.discussion_repository
            .get_by_chat_room_user(user_id)
            .await
    }

    pub async fn create(&self, data: CreateDiscussion) -> CtxResult<Discussion> {
        data.validate()?;

        let user_id = self.ctx.user_id()?;
        let user_thing: Thing = get_str_thing(&user_id)?;
        let comm_id = self
            .access_right_repository
            .has_owner_access(&user_thing, &data.community_id)
            .await?;

        if data.private_discussion_users_final && data.chat_user_ids.is_some() {
            let mut ids = data
                .chat_user_ids
                .as_ref()
                .unwrap()
                .iter()
                .map(|t| t.as_str())
                .collect::<Vec<&str>>();

            if !ids.contains(&user_id.as_str()) {
                ids.push(&user_id);
            };

            let res = self.discussion_repository.get_by_private_users(ids).await;

            match res {
                Ok(value) => {
                    return Ok(value);
                }
                Err(_) => (),
            }
        };

        let private_discussion_user_ids = match data.chat_user_ids {
            Some(ids) => {
                let mut user_ids = record_exist_all(self.user_repository.db, ids).await?;
                if !user_ids.contains(&user_thing) {
                    user_ids.push(user_thing.clone());
                }
                Some(user_ids)
            }
            None => None,
        };

        let disc = self
            .discussion_repository
            .create_update(Discussion {
                id: None,
                belongs_to: comm_id,
                title: Some(data.title.clone()),
                image_uri: None,
                topics: None,
                private_discussion_user_ids,
                latest_post_id: None,
                r_created: None,
                created_by: user_thing.clone(),
                private_discussion_users_final: data.private_discussion_users_final,
            })
            .await?;

        Ok(disc)
    }
}
