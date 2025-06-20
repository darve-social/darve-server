use serde::{Deserialize, Serialize};
use surrealdb::sql::Thing;
use validator::Validate;
use crate::middleware::utils::db_utils::{record_exist_all, IdentIdName};
use crate::{
    entities::{
        community::{
            community_entity::CommunityDbService,
            discussion_entity::{Discussion, DiscussionDbService},
        },
        user_auth::local_user_entity::LocalUserDbService,
    },
    middleware::{
        ctx::Ctx,
        error::{AppError, AppResult, CtxResult},
        mw_ctx::CtxState,
    },
};
use crate::entities::user_auth::access_right_entity::AccessRightDbService;
use crate::middleware::error::CtxError;
use crate::middleware::utils::string_utils::{get_str_thing, get_string_thing};

#[derive(Deserialize, Serialize, Validate)]
pub struct CreateDiscussion {
    pub community_id: String,
    #[validate(length(min = 5, message = "Min 5 characters"))]
    pub title: String,
    pub image_uri: Option<String>,
    pub chat_user_ids: Option<Vec<String>>,
    pub is_chat_users_final: bool,
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

    pub async fn delete( &self,  disc_id: &str) -> AppResult<()> {
        let user_id: Thing = get_str_thing( self.ctx.user_id()?.as_str())?;
        self.access_right_repository.has_owner_access(&user_id, disc_id).await?;

        self.discussion_repository.delete(disc_id).await?;
        Ok(())
    }

    pub async fn update(
        &self,
        disc_id: &str,
        data: UpdateDiscussion,
    ) -> AppResult<()> {
        let user_id: Thing = get_str_thing( self.ctx.user_id()?.as_str())?;
        let disc_id = self.access_right_repository.has_owner_access(&user_id, disc_id).await?;

        let mut disc = self.discussion_repository.get(IdentIdName::Id(disc_id)).await?;
        disc.title = data.title;
        self.discussion_repository.create_update(disc).await?;

        Ok(())
    }

    pub async fn add_chat_users(
        &self,
        disc_id: &str,
        new_user_ids: Vec<String>,
    ) -> CtxResult<Vec<Thing>> {
        
        if new_user_ids.is_empty() {
            return Err(self.ctx.to_ctx_error(AppError::Generic {description:"no users present".to_string()}));
        }

        let user_id: Thing = get_str_thing( self.ctx.user_id()?.as_str())?;
        let disc_id = self.access_right_repository.has_owner_access(&user_id, disc_id).await?;

        let mut disc = self.discussion_repository.get(IdentIdName::Id(disc_id)).await?;
        
        let user_ids = record_exist_all(self.user_repository.db, new_user_ids).await?;
        
        let mut chat_users = disc.chat_room_user_ids.unwrap_or(vec![]);

        user_ids.into_iter().for_each(|id| {
            if !chat_users.contains(&id) {
                chat_users.push(id);
            }
        });

        disc.chat_room_user_ids = Some(chat_users);
        let chat_users = self.discussion_repository.create_update(disc).await?.chat_room_user_ids.expect("users are set");

        Ok(chat_users)
    }

    pub async fn remove_chat_users(
        &self,
        disc_id: &str,
        remove_user_ids: Vec<String>,
    ) -> CtxResult<()> {
        
        if remove_user_ids.is_empty() {
            return Err(self.ctx.to_ctx_error(AppError::Generic {description:"no users present".to_string()}));
        }

        let user_id: Thing = get_str_thing( self.ctx.user_id()?.as_str())?;
        let disc_id = self.access_right_repository.has_owner_access(&user_id, disc_id).await?;

        let mut disc = self.discussion_repository.get(IdentIdName::Id(disc_id)).await?;

        if disc.is_chat_users_final {
            return Err(AppError::Generic {
                description: "Forbidden".to_string(),
            }
            .into());
        };

        if disc.chat_room_user_ids.is_none() || disc.chat_room_user_ids.as_ref().unwrap().is_empty() {
            return Err(AppError::Generic {
                description: "Forbidden".to_string(),
            }
            .into());
        };

        let mut remove_things =vec![];
        for id in remove_user_ids.iter() {
            match Thing::try_from(id.as_str()) {
                Ok(v) => {
                    if v == user_id {
                        return Err(self.ctx.to_ctx_error(AppError::Generic {
                            description: "Owner of the discussion can not remove yourself".to_string(),
                        }));
                    }
                    remove_things.push(v);
                },
                Err(_) => {
                    return Err(AppError::Generic {
                        description: "Invalid user id".to_string(),
                    }
                    .into());
                },
            };            
        };

        if remove_things.is_empty()  {
            return Ok(());
        }

        let chat_room_user_ids = disc
            .chat_room_user_ids
            .unwrap()
            .into_iter()
            .filter(|id| !remove_things.contains(id))
            .collect::<Vec<Thing>>();

        disc.chat_room_user_ids = Some(chat_room_user_ids);
        self.discussion_repository.create_update(disc).await?;

        Ok(())
    }

    pub async fn get_by_chat_user(&self, user_id: &str) -> CtxResult<Vec<Discussion>> {
        self.discussion_repository.get_by_chat_room_user(user_id).await
    }

    pub async fn create(&self, data: CreateDiscussion) -> CtxResult<Discussion> {
        data.validate()?;

        let user_id = self.ctx.user_id()?;
        let user_thing: Thing = get_str_thing(&user_id)?;
        let comm_id = self.access_right_repository.has_owner_access(&user_thing, &data.community_id).await?;

        if data.is_chat_users_final && data.chat_user_ids.is_some() {
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

            let res = self
                .discussion_repository
                .get_by_read_only(ids, Some(data.title.clone()))
                .await;

            match res {
                Ok(value) => {
                    return Ok(value);
                }
                Err(_) => (),
            }
        };

        let chat_room_user_ids = match data.chat_user_ids {
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
                chat_room_user_ids,
                latest_post_id: None,
                r_created: None,
                created_by: user_thing.clone(),
                is_chat_users_final: data.is_chat_users_final,
            })
            .await?;

        Ok(disc)
    }
}
