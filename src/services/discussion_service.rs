use serde::{Deserialize, Serialize};
use surrealdb::sql::Thing;
use validator::Validate;
use darve_server::middleware::utils::db_utils::record_exist_all;
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

#[derive(Deserialize, Serialize, Validate)]
pub struct CreateDiscussion {
    pub community_id: String,
    #[validate(length(min = 5, message = "Min 5 characters"))]
    pub title: String,
    pub image_uri: Option<String>,
    pub user_ids: Option<Vec<String>>,
    pub is_read_only: bool,
}

#[derive(Debug, Deserialize, Serialize, Validate)]
pub struct UpdateDiscussion {
    pub title: Option<String>,
}

pub struct DiscussionService<'a> {
    user_repository: LocalUserDbService<'a>,
    comm_repository: CommunityDbService<'a>,
    discussion_repository: DiscussionDbService<'a>,
    access_right_repository: AccessRightDbService<'a>,
}

impl<'a> DiscussionService<'a> {
    pub fn new(state: &'a CtxState, ctx: &'a Ctx) -> Self {
        Self {
            user_repository: LocalUserDbService {
                db: &state.db.client,
                ctx: &ctx,
            },
            comm_repository: CommunityDbService {
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

    pub async fn delete(&self, disc_id: &str, user_id: &str) -> AppResult<()> {
        let disc = self.discussion_repository.get_by_id(&disc_id).await?;

        if disc.created_by.to_raw() != user_id {
            return Err(AppError::Generic {
                description: "Forbidden".to_string(),
            }
            .into());
        };

        self.discussion_repository.delete(&disc_id).await?;
        Ok(())
    }

    pub async fn update(
        &self,
        disc_id: &str,
        user_id: &str,
        data: UpdateDiscussion,
    ) -> AppResult<()> {
        let mut disc = self.discussion_repository.get_by_id(&disc_id).await?;

        if disc.created_by.to_raw() != user_id || disc.is_read_only {
            return Err(AppError::Generic {
                description: "Forbidden".to_string(),
            }
            .into());
        };

        disc.title = data.title;
        self.discussion_repository.create_update(disc).await?;

        Ok(())
    }

    pub async fn add_chat_users(
        &self,
        disc_id: &str,
        user_id: &str,
        new_user_ids: Vec<String>,
    ) -> CtxResult<()> {
        let mut disc = self.discussion_repository.get_by_id(&disc_id).await?;

        if disc.created_by.to_raw() != user_id || disc.is_read_only {
            return Err(AppError::Generic {
                description: "Forbidden".to_string(),
            }
            .into());
        };
        
        let users_exist = record_exist_all(self.user_repository.db, new_user_ids)

        let user_ids = self
            .user_repository
            .get_by_ids(new_user_ids)
            .await?
            .into_iter()
            .map(|u| u.id.as_ref().unwrap().clone())
            .collect::<Vec<Thing>>();

        let mut chat_users = disc.chat_room_user_ids.unwrap_or(vec![]);

        user_ids.into_iter().for_each(|id| {
            if !chat_users.contains(&id) {
                chat_users.push(id);
            }
        });

        disc.chat_room_user_ids = Some(chat_users);
        self.discussion_repository.create_update(disc).await?;

        Ok(())
    }

    pub async fn remove_chat_users(
        &self,
        disc_id: &str,
        user_id: &str,
        remove_user_ids: Vec<String>,
    ) -> CtxResult<()> {
        let mut disc = self.discussion_repository.get_by_id(&disc_id).await?;

        if disc.created_by.to_raw() != user_id || disc.is_read_only {
            return Err(AppError::Generic {
                description: "Forbidden".to_string(),
            }
            .into());
        };

        if disc.chat_room_user_ids.is_none() {
            return Err(AppError::Generic {
                description: "Forbidden".to_string(),
            }
            .into());
        };

        if remove_user_ids.contains(&user_id.to_string()) {
            return Err(AppError::Generic {
                description: "Owner of the discussion can not remove yourself".to_string(),
            }
            .into());
        };

        let things = remove_user_ids.iter().fold(vec![], |mut res, id| {
            match Thing::try_from(id.as_str()) {
                Ok(v) => res.push(v),
                Err(_) => (),
            };
            res
        });

        if things.is_empty() || disc.chat_room_user_ids.is_none() {
            return Ok(());
        }

        let chat_room_user_ids = disc
            .chat_room_user_ids
            .unwrap()
            .into_iter()
            .filter(|id| !things.contains(id))
            .collect::<Vec<Thing>>();

        disc.chat_room_user_ids = Some(chat_room_user_ids);
        self.discussion_repository.create_update(disc).await?;

        Ok(())
    }

    pub async fn get_by_chat_user(&self, user_id: &str) -> CtxResult<Vec<Discussion>> {
        self.discussion_repository.get_by_user(user_id).await
    }

    pub async fn create(&self, user_id: &str, data: CreateDiscussion) -> CtxResult<Discussion> {
        data.validate()?;

        let user_thing = Thing::try_from(user_id).map_err(|_| AppError::Generic {
            description: "error into Thing".to_string(),
        })?;

        let comm_id = self.access_right_repository.has_owner_access(&data.community_id).await?;

        if data.is_read_only && data.user_ids.is_some() {
            let mut ids = data
                .user_ids
                .as_ref()
                .unwrap()
                .iter()
                .map(|t| t.as_str())
                .collect::<Vec<&str>>();

            if !ids.contains(&user_id) {
                ids.push(user_id);
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

        let chat_room_user_ids = match data.user_ids {
            Some(ids) => {
                let ids = ids
                    .into_iter()
                    .filter(|v| *v != *user_id)
                    .collect::<Vec<String>>();
                let users = self.user_repository.get_by_ids(ids).await?;
                let mut user_ids = users
                    .iter()
                    .map(|v| v.id.as_ref().unwrap().clone())
                    .collect::<Vec<Thing>>();
                user_ids.push(user_thing.clone());
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
                is_read_only: data.is_read_only,
            })
            .await?;

        Ok(disc)
    }
}
