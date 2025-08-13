use crate::access::community::CommunityAccess;
use crate::access::discussion::DiscussionAccess;
use crate::entities::community::community_entity::CommunityDbService;
use crate::entities::community::discussion_entity::{CreateDiscussionEntity, DiscussionDenyRule};
use crate::middleware::utils::db_utils::record_exist_all;
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
    community_repository: CommunityDbService<'a>,
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
            community_repository: CommunityDbService {
                db: &state.db.client,
                ctx: &ctx,
            },
        }
    }

    pub async fn delete(&self, user_id: &str, disc_id: &str) -> AppResult<()> {
        let user = self.user_repository.get_by_id(&user_id).await?;
        let disc = self.discussion_repository.get_by_id(&disc_id).await?;
        if !DiscussionAccess::new(&disc).can_edit(&user) {
            return Err(AppError::Forbidden);
        }
        self.discussion_repository.delete(disc_id).await?;
        Ok(())
    }

    pub async fn update(
        &self,
        user_id: &str,
        disc_id: &str,
        data: UpdateDiscussion,
    ) -> AppResult<()> {
        let user = self.user_repository.get_by_id(&user_id).await?;
        let disc = self.discussion_repository.get_by_id(&disc_id).await?;
        if !DiscussionAccess::new(&disc).can_edit(&user) {
            return Err(AppError::Forbidden);
        }

        self.discussion_repository
            .update(
                &disc.id.as_ref().unwrap().id.to_raw(),
                &data.title.unwrap_or("".to_string()),
            )
            .await?;

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

        let user = self.user_repository.get_by_id(&user_id).await?;
        let disc = self.discussion_repository.get_by_id(&disc_id).await?;
        if !DiscussionAccess::new(&disc).can_manage_members(&user) {
            return Err(self.ctx.to_ctx_error(AppError::Forbidden));
        }

        let user_ids = record_exist_all(self.user_repository.db, new_user_ids).await?;

        let mut chat_users = disc.private_discussion_user_ids.unwrap_or(vec![]);

        user_ids.into_iter().for_each(|id| {
            if !chat_users.contains(&id) {
                chat_users.push(id);
            }
        });

        let users = self
            .discussion_repository
            .update_users(&disc.id.as_ref().unwrap().id.to_raw(), Some(chat_users))
            .await?
            .private_discussion_user_ids
            .expect("users are set");

        Ok(users)
    }

    pub async fn remove_chat_users(
        &self,
        user_id: &str,
        disc_id: &str,
        remove_user_ids: Vec<String>,
    ) -> CtxResult<()> {
        if remove_user_ids.is_empty() {
            return Err(self.ctx.to_ctx_error(AppError::Generic {
                description: "no users present".to_string(),
            }));
        }
        let user = self.user_repository.get_by_id(&user_id).await?;
        let user_id_str = user.id.as_ref().unwrap().to_raw();

        if remove_user_ids.contains(&user_id_str) {
            return Err(self.ctx.to_ctx_error(AppError::Generic {
                description: "Owner of the discussion can not remove yourself".to_string(),
            }));
        };

        let disc = self.discussion_repository.get_by_id(&disc_id).await?;

        if !DiscussionAccess::new(&disc).can_manage_members(&user) {
            return Err(self.ctx.to_ctx_error(AppError::Forbidden));
        }

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

        self.discussion_repository
            .update_users(
                &disc.id.as_ref().unwrap().id.to_raw(),
                Some(private_discussion_user_ids),
            )
            .await?;

        Ok(())
    }

    pub async fn get_by_chat_user(&self, user_id: &str) -> CtxResult<Vec<Discussion>> {
        self.discussion_repository
            .get_by_chat_room_user(user_id)
            .await
    }

    pub async fn create(&self, user_id: &str, data: CreateDiscussion) -> CtxResult<Discussion> {
        data.validate()?;
        let user = self.user_repository.get_by_id(&user_id).await?;

        let comm = self
            .community_repository
            .get_by_id(&data.community_id)
            .await?;

        if !CommunityAccess::new(&comm).can_create_discussion(&user) {
            return Err(self.ctx.to_ctx_error(AppError::Forbidden));
        }

        if data.private_discussion_users_final && data.chat_user_ids.is_some() {
            let mut ids = data.chat_user_ids.as_ref().unwrap().clone();

            let user_id = user.id.as_ref().unwrap().to_raw();
            if !ids.contains(&user_id) {
                ids.push(user_id.clone());
            };

            let ids_ref: Vec<&str> = ids.iter().map(|s| s.as_str()).collect();
            let res = self
                .discussion_repository
                .get_by_private_users(ids_ref)
                .await;
            println!("res: {:?}", res);

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
                if !user_ids.contains(&user.id.as_ref().unwrap()) {
                    user_ids.push(user.id.as_ref().unwrap().clone());
                }
                Some(user_ids)
            }
            None => None,
        };

        let deny_rules = if data.private_discussion_users_final {
            DiscussionDenyRule::private_fixed()
        } else if private_discussion_user_ids.is_some() {
            DiscussionDenyRule::private()
        } else {
            DiscussionDenyRule::public()
        };

        let disc = self
            .discussion_repository
            .create(CreateDiscussionEntity {
                belongs_to: comm.id.clone(),
                title: data.title,
                image_uri: None,
                private_discussion_user_ids,
                created_by: user.id.as_ref().unwrap().clone(),
                deny_rules,
            })
            .await?;

        Ok(disc)
    }
}
