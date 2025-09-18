use std::hash::{DefaultHasher, Hash, Hasher};

use crate::access::base::role::Role;
use crate::access::community::CommunityAccess;
use crate::access::discussion::DiscussionAccess;
use crate::entities::access_user::AccessUser;
use crate::entities::community::community_entity::CommunityDbService;
use crate::entities::community::discussion_entity::{
    CreateDiscussionEntity, DiscussionType, TABLE_NAME as DISC_TABLE_NAME,
};
use crate::interfaces::repositories::access::AccessRepositoryInterface;
use crate::interfaces::repositories::discussion_user::DiscussionUserRepositoryInterface;
use crate::middleware::utils::db_utils::{record_exist_all, Pagination};
use crate::middleware::utils::string_utils::get_str_thing;
use crate::models::view::access::DiscussionAccessView;
use crate::models::view::discussion::DiscussionView;
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
    #[serde(default)]
    pub private_discussion_users_final: bool,
}

#[derive(Debug, Deserialize, Serialize, Validate)]
pub struct UpdateDiscussion {
    pub title: Option<String>,
}

pub struct DiscussionService<'a, A, U>
where
    U: DiscussionUserRepositoryInterface,
    A: AccessRepositoryInterface,
{
    ctx: &'a Ctx,
    user_repository: LocalUserDbService<'a>,
    discussion_repository: DiscussionDbService<'a>,
    community_repository: CommunityDbService<'a>,
    access_repository: &'a A,
    discussion_users: &'a U,
}

impl<'a, A, U> DiscussionService<'a, A, U>
where
    U: DiscussionUserRepositoryInterface,
    A: AccessRepositoryInterface,
{
    pub fn new(
        state: &'a CtxState,
        ctx: &'a Ctx,
        access_repository: &'a A,
        discussion_users: &'a U,
    ) -> Self {
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
            access_repository,
            discussion_users,
        }
    }

    pub async fn delete(&self, user_id: &str, disc_id: &str) -> AppResult<()> {
        let user = self.user_repository.get_by_id(&user_id).await?;
        let disc = self
            .discussion_repository
            .get_view_by_id::<DiscussionAccessView>(&disc_id)
            .await?;
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
        let disc = self
            .discussion_repository
            .get_view_by_id::<DiscussionAccessView>(&disc_id)
            .await?;

        if !DiscussionAccess::new(&disc).can_edit(&user) {
            return Err(AppError::Forbidden);
        }

        self.discussion_repository
            .update(&disc.id.id.to_raw(), &data.title.unwrap_or("".to_string()))
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
        let disc = self
            .discussion_repository
            .get_view_by_id::<DiscussionAccessView>(&disc_id)
            .await?;

        if !DiscussionAccess::new(&disc).can_add_member(&user) {
            return Err(self.ctx.to_ctx_error(AppError::Forbidden));
        }

        let user_ids = record_exist_all(self.user_repository.db, new_user_ids).await?;
        let disc_user_ids = disc
            .users
            .into_iter()
            .map(|u| u.user)
            .collect::<Vec<Thing>>();

        let new_users = user_ids
            .into_iter()
            .filter(|id| !disc_user_ids.contains(id))
            .collect::<Vec<Thing>>();

        let disc_id = disc.id.id.to_raw();
        self.access_repository
            .add(new_users.clone(), vec![disc.id], Role::Member.to_string())
            .await?;

        self.discussion_users
            .create(&disc_id, new_users.clone())
            .await?;

        Ok(new_users)
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

        let disc = self
            .discussion_repository
            .get_view_by_id::<DiscussionAccessView>(&disc_id)
            .await?;

        if !DiscussionAccess::new(&disc).can_remove_member(&user) {
            return Err(self.ctx.to_ctx_error(AppError::Forbidden));
        }
        let user_things = remove_user_ids
            .iter()
            .filter_map(|u| get_str_thing(u).ok())
            .collect::<Vec<Thing>>();

        let disc_id = disc.id.id.to_raw();
        self.access_repository
            .remove_by_entity(disc.id, user_things.clone())
            .await?;

        self.discussion_users.remove(&disc_id, user_things).await?;

        Ok(())
    }

    pub async fn get(
        &self,
        user_id: &str,
        disc_type: Option<DiscussionType>,
        pag: Pagination,
    ) -> CtxResult<Vec<DiscussionView>> {
        let _ = self.user_repository.get_by_id(&user_id).await?;

        self.discussion_repository
            .get_by_type(user_id, disc_type, pag)
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

        let private_discussion_user_ids = match data.chat_user_ids {
            Some(ids) => {
                let user_id_str = user.id.as_ref().unwrap().id.to_raw();
                let user_ids = ids
                    .into_iter()
                    .filter(|u| u != &user_id_str)
                    .collect::<Vec<String>>();
                Some(record_exist_all(self.user_repository.db, user_ids).await?)
            }
            None => None,
        };

        let (disc_id, disc_type, owner_role) =
            if data.private_discussion_users_final && private_discussion_user_ids.is_some() {
                let mut ids = private_discussion_user_ids
                    .as_ref()
                    .unwrap()
                    .iter()
                    .map(|id| id.id.to_raw())
                    .collect::<Vec<String>>();

                ids.push(user_id.to_string());

                ids.sort();

                let mut hasher = DefaultHasher::new();
                ids.join("").hash(&mut hasher);
                let hash_id = hasher.finish();

                let id = Thing::from((DISC_TABLE_NAME, format!("{:x}", hash_id).as_str()));
                let res = self
                    .discussion_repository
                    .get_view_by_id::<DiscussionView>(&id.to_raw())
                    .await;

                if let Ok(disc) = res {
                    let access_view = DiscussionAccessView {
                        id: disc.id.clone(),
                        r#type: disc.r#type.clone(),
                        users: disc
                            .users
                            .into_iter()
                            .map(|v| AccessUser {
                                role: v.role,
                                user: v.user.id,
                                created_at: v.created_at,
                            })
                            .collect::<Vec<AccessUser>>(),
                    };
                    if !DiscussionAccess::new(&access_view).can_view(&user) {
                        return Err(self.ctx.to_ctx_error(AppError::Forbidden));
                    }

                    return Ok(Discussion {
                        id: disc.id,
                        belongs_to: disc.belongs_to,
                        title: disc.title,
                        image_uri: disc.image_uri,
                        created_at: disc.created_at,
                        updated_at: disc.updated_at,
                        created_by: disc.created_by.id,
                        r#type: disc.r#type,
                    });
                }
                (Some(id), DiscussionType::Private, Role::Editor)
            } else {
                (
                    None,
                    if private_discussion_user_ids.is_some() {
                        DiscussionType::Private
                    } else {
                        DiscussionType::Public
                    },
                    Role::Owner,
                )
            };

        let disc = self
            .discussion_repository
            .create(CreateDiscussionEntity {
                id: disc_id,
                belongs_to: comm.id.clone(),
                title: data.title,
                image_uri: None,
                created_by: user.id.as_ref().unwrap().clone(),
                r#type: disc_type,
            })
            .await?;

        self.access_repository
            .add(
                [user.id.as_ref().unwrap().clone()].to_vec(),
                [disc.id.clone()].to_vec(),
                owner_role.to_string(),
            )
            .await?;

        if let Some(mut user_ids) = private_discussion_user_ids {
            self.access_repository
                .add(
                    user_ids.clone(),
                    [disc.id.clone()].to_vec(),
                    Role::Member.to_string(),
                )
                .await?;
            user_ids.push(user.id.as_ref().unwrap().clone());
            self.discussion_users
                .create(&disc.id.id.to_raw(), user_ids)
                .await?;
        }
        Ok(disc)
    }
}
