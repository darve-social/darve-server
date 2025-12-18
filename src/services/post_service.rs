use std::sync::Arc;

use crate::{
    access::{base::role::Role, discussion::DiscussionAccess, post::PostAccess},
    database::client::Db,
    entities::{
        community::{
            discussion_entity::{DiscussionDbService, DiscussionType},
            post_entity::{CreatePost, PostDbService, PostType},
        },
        user_auth::local_user_entity::{LocalUser, LocalUserDbService},
    },
    interfaces::{
        file_storage::FileStorageInterface,
        repositories::{
            access::AccessRepositoryInterface, discussion_user::DiscussionUserRepositoryInterface,
            like::LikesRepositoryInterface, tags::TagsRepositoryInterface,
            user_notifications::UserNotificationsInterface,
        },
    },
    middleware::{
        ctx::Ctx,
        error::{AppError, AppResult, CtxResult},
        mw_ctx::AppEvent,
        utils::{
            db_utils::{Pagination, QryOrder},
            string_utils::get_str_thing,
        },
    },
    models::view::{
        access::{DiscussionAccessView, PostAccessView},
        full_post::FullPostView,
        post::{PostUsersView, PostView},
        user::UserView,
    },
    services::notification_service::NotificationService,
    utils::{
        file::convert::{convert_field_file_data, FileUpload},
        validate_utils::validate_tags,
    },
};

use axum_typed_multipart::{FieldData, TryFromMultipart};
use futures::future::join_all;
use serde::Deserialize;
use surrealdb::sql::Thing;
use tempfile::NamedTempFile;
use tokio::sync::broadcast::Sender;
use validator::Validate;

#[derive(Debug, Deserialize)]
pub struct GetPostsParams {
    pub filter_by_type: Option<PostType>,
    pub order_dir: Option<QryOrder>,
    pub start: Option<u32>,
    pub count: Option<u16>,
}

#[derive(Debug, Deserialize, Validate)]
pub struct PostLikeData {
    #[validate(range(min = 2, max = 10))]
    pub count: Option<u16>,
}

#[derive(Debug, Validate, TryFromMultipart)]
pub struct PostInput {
    #[validate(length(min = 5, message = "Min 5 characters"))]
    pub title: String,
    #[validate(length(min = 1, message = "Content cannot be empty"))]
    pub content: Option<String>,
    #[validate(length(max = 5, message = "Max 5 tags"))]
    #[validate(custom(function=validate_tags))]
    pub tags: Vec<String>,
    #[form_data(limit = "unlimited")]
    pub file_1: Option<FieldData<NamedTempFile>>,
    pub is_idea: Option<bool>,
    pub users: Vec<String>,
}

pub struct PostService<'a, N, T, L, A, DU>
where
    N: UserNotificationsInterface,
    T: TagsRepositoryInterface,
    L: LikesRepositoryInterface,
    A: AccessRepositoryInterface,
    DU: DiscussionUserRepositoryInterface,
{
    users_repository: LocalUserDbService<'a>,
    discussions_repository: DiscussionDbService<'a>,
    posts_repository: PostDbService<'a>,
    file_storage: Arc<dyn FileStorageInterface + Send + Sync>,
    likes_repository: &'a L,
    notification_service: NotificationService<'a, N>,
    tags_repository: &'a T,
    access_repository: &'a A,
    discussion_users: &'a DU,
}

impl<'a, N, T, L, A, DU> PostService<'a, N, T, L, A, DU>
where
    N: UserNotificationsInterface,
    T: TagsRepositoryInterface,
    L: LikesRepositoryInterface,
    A: AccessRepositoryInterface,
    DU: DiscussionUserRepositoryInterface,
{
    pub fn new(
        db: &'a Db,
        ctx: &'a Ctx,
        event_sender: &'a Sender<AppEvent>,
        notification_repository: &'a N,
        file_storage: Arc<dyn FileStorageInterface + Send + Sync>,
        tags_repository: &'a T,
        likes_repository: &'a L,
        access_repository: &'a A,
        discussion_users: &'a DU,
    ) -> Self {
        Self {
            users_repository: LocalUserDbService { db: &db, ctx: &ctx },
            posts_repository: PostDbService { db: &db, ctx: &ctx },
            notification_service: NotificationService::new(
                db,
                ctx,
                event_sender,
                notification_repository,
            ),
            discussions_repository: DiscussionDbService { db: &db, ctx },
            file_storage,
            tags_repository,
            likes_repository,
            access_repository,
            discussion_users,
        }
    }

    pub async fn like(&self, post_id: &str, user_id: &str, data: PostLikeData) -> CtxResult<u32> {
        data.validate()?;
        let user = self.users_repository.get_by_id(&user_id).await?;
        let post = self
            .posts_repository
            .get_view_by_id::<PostAccessView>(post_id, None)
            .await?;

        let likes = data.count.unwrap_or(1);
        let by_credits = data.count.is_some();

        if !PostAccess::new(&post).can_like(&user) {
            return Err(AppError::Forbidden.into());
        }

        if by_credits && user.credits < likes as u64 {
            return Err(AppError::Generic {
                description: "The user does not have enough credits".to_string(),
            }
            .into());
        }

        let likes_count = self
            .likes_repository
            .like(user.id.as_ref().unwrap().clone(), post.id.clone(), likes)
            .await?;

        self.notification_service
            .on_post_like(&user, &post, likes == 10)
            .await?;

        if by_credits {
            self.users_repository
                .remove_credits(user.id.as_ref().unwrap().clone(), likes)
                .await?;
        }

        Ok(likes_count)
    }

    pub async fn unlike(&self, post_id: &str, user_id: &str) -> CtxResult<u32> {
        let user = self.users_repository.get_by_id(&user_id).await?;
        let post = self
            .posts_repository
            .get_view_by_id::<PostAccessView>(post_id, None)
            .await?;

        if !PostAccess::new(&post).can_like(&user) {
            return Err(AppError::Forbidden.into());
        }

        let likes_count = self
            .likes_repository
            .unlike(user.id.as_ref().unwrap().clone(), post.id.clone())
            .await?;

        Ok(likes_count)
    }

    pub async fn add_members(
        &self,
        user_id: &str,
        post_id: &str,
        user_ids: Vec<String>,
    ) -> CtxResult<()> {
        let user = self.users_repository.get_by_id(user_id).await?;

        let post = self
            .posts_repository
            .get_view_by_id::<PostAccessView>(post_id, None)
            .await?;

        let post_access = PostAccess::new(&post);
        if !post_access.can_add_member(&user) {
            return Err(AppError::Forbidden.into());
        }

        let disc_access = DiscussionAccess::new(&post.discussion);
        let members = self.get_users_by_ids(user_ids).await?;
        if !members.iter().all(|u| disc_access.can_view(u)) {
            return Err(AppError::Forbidden.into());
        }
        let post_user_ids = post
            .users
            .into_iter()
            .map(|u| u.user)
            .collect::<Vec<Thing>>();

        let new_members = members
            .into_iter()
            .filter(|u| !post_user_ids.contains(u.id.as_ref().unwrap()))
            .map(|u| u.id.as_ref().unwrap().clone())
            .collect::<Vec<Thing>>();

        if new_members.is_empty() {
            return Ok(());
        }

        let _ = self
            .access_repository
            .add(new_members, vec![post.id.clone()], Role::Member.to_string())
            .await?;

        Ok(())
    }

    pub async fn remove_members(
        &self,
        user_id: &str,
        post_id: &str,
        user_ids: Vec<String>,
    ) -> CtxResult<()> {
        let user = self.users_repository.get_by_id(user_id).await?;
        let user_id_str = user.id.as_ref().unwrap().to_raw();
        if user_ids.contains(&user_id_str) {
            return Err(AppError::Generic {
                description: "Owner of the post can not remove yourself".to_string(),
            }
            .into());
        };

        let post = self
            .posts_repository
            .get_view_by_id::<PostAccessView>(post_id, None)
            .await?;

        let post_access = PostAccess::new(&post);
        if !post_access.can_remove_member(&user) {
            return Err(AppError::Forbidden.into());
        }

        let user_things = user_ids
            .into_iter()
            .filter_map(|v| get_str_thing(&v).ok())
            .collect::<Vec<Thing>>();

        let post_user_ids = post
            .users
            .into_iter()
            .map(|u| u.user)
            .collect::<Vec<Thing>>();

        let remove_members = user_things
            .into_iter()
            .filter(|u| post_user_ids.contains(u))
            .collect::<Vec<Thing>>();

        if remove_members.is_empty() {
            return Ok(());
        }

        let disc_users = remove_members
            .iter()
            .map(|id| id.id.to_raw())
            .collect::<Vec<String>>();

        let _ = self
            .access_repository
            .remove_by_entity(post.id.clone(), remove_members)
            .await?;

        let updated_discs_users = self
            .discussion_users
            .update_latest_post(&post.discussion.id.id.to_raw(), disc_users)
            .await?;
        let _ = self
            .notification_service
            .on_updated_users_discussions(&user.id.as_ref().unwrap(), &updated_discs_users)
            .await?;

        Ok(())
    }

    pub async fn get_by_disc(
        &self,
        disc_id: &str,
        user_id: &str,
        query: GetPostsParams,
    ) -> CtxResult<Vec<PostView>> {
        let user = self.users_repository.get_by_id(&user_id).await?;
        let disc = self
            .discussions_repository
            .get_view_by_id::<DiscussionAccessView>(disc_id)
            .await?;

        if !DiscussionAccess::new(&disc).can_view(&user) {
            return Err(AppError::Forbidden.into());
        }

        let pagination = Pagination {
            order_by: None,
            order_dir: None,
            count: query.count.unwrap_or(20),
            start: query.start.unwrap_or(0),
        };

        let items = self
            .posts_repository
            .get_by_disc(
                &user.id.as_ref().unwrap().id.to_raw(),
                &disc.id.id.to_raw(),
                query.filter_by_type,
                pagination,
            )
            .await?;

        Ok(items)
    }

    pub async fn get_count(
        &self,
        disc_id: &str,
        user_id: &str,
        filter_by_type: Option<PostType>,
    ) -> CtxResult<u64> {
        let user = self.users_repository.get_by_id(&user_id).await?;
        let disc = self
            .discussions_repository
            .get_view_by_id::<DiscussionAccessView>(disc_id)
            .await?;

        if !DiscussionAccess::new(&disc).can_view(&user) {
            return Err(AppError::Forbidden.into());
        }

        let count = self
            .posts_repository
            .get_count(
                &user.id.as_ref().unwrap().id.to_raw(),
                &disc.id.id.to_raw(),
                filter_by_type,
            )
            .await?;

        Ok(count)
    }

    pub async fn get_users(&self, post_id: &str, user_id: &str) -> CtxResult<Vec<UserView>> {
        let user = self.users_repository.get_by_id(&user_id).await?;
        let post = self
            .posts_repository
            .get_view_by_id::<PostAccessView>(post_id, None)
            .await?;
        if !PostAccess::new(&post).can_view(&user) {
            return Err(AppError::Forbidden.into());
        }

        let post = self
            .posts_repository
            .get_view_by_id::<PostUsersView>(post_id, None)
            .await?;

        Ok(post
            .users
            .unwrap_or_default()
            .into_iter()
            .map(|a| a.user)
            .collect::<Vec<UserView>>())
    }

    pub async fn create(
        &self,
        user_id: &str,
        disc_id: &str,
        data: PostInput,
    ) -> CtxResult<PostView> {
        let post_data = self.get_post_data_of_input(data).await?;
        let user = self.users_repository.get_by_id(user_id).await?;

        let disc = self
            .discussions_repository
            .get_view_by_id::<DiscussionAccessView>(disc_id)
            .await?;

        self.check_create_access(&disc, &post_data, &user)?;

        let media_links = if let Some(file) = post_data.file {
            let file_name = format!(
                "{}_{}",
                post_data.id.to_raw().replace(":", "_"),
                file.file_name
            );
            let result = self
                .file_storage
                .upload(
                    file.data,
                    Some("posts"),
                    &file_name,
                    file.content_type.as_deref(),
                )
                .await
                .map_err(|e| AppError::Generic { description: e })?;

            Some(vec![result])
        } else {
            None
        };

        let post_res = self
            .posts_repository
            .create(CreatePost {
                belongs_to: disc.id.clone(),
                title: post_data.title,
                content: post_data.content,
                media_links: media_links.clone(),
                created_by: user.id.as_ref().unwrap().clone(),
                id: post_data.id,
                r#type: post_data.r#type.clone(),
            })
            .await;

        let post = match post_res {
            Ok(value) => value,
            Err(err) => {
                if let Some(links) = &media_links {
                    let futures = links.into_iter().map(|link| {
                        let file_storage = self.file_storage.clone();
                        async move {
                            if let Some(filename) = link.split('/').last() {
                                let _ = file_storage.delete(Some("posts"), filename).await;
                            }
                        }
                    });

                    join_all(futures).await;
                }
                return Err(err);
            }
        };

        let _ = self
            .access_repository
            .add(
                vec![user.id.as_ref().unwrap().clone()],
                vec![post.id.as_ref().unwrap().clone()],
                Role::Owner.to_string(),
            )
            .await?;
        let member_ids = post_data
            .members
            .iter()
            .filter_map(|u| {
                if u.id.as_ref() == user.id.as_ref() {
                    None
                } else {
                    Some(u.id.as_ref().unwrap().clone())
                }
            })
            .collect::<Vec<Thing>>();

        if !member_ids.is_empty() {
            let _ = self
                .access_repository
                .add(
                    member_ids.clone(),
                    vec![post.id.as_ref().unwrap().clone()],
                    Role::Member.to_string(),
                )
                .await?;
        }

        let disc_all_users = match post_data.r#type {
            PostType::Private => member_ids
                .iter()
                .map(|id| id.id.to_raw())
                .chain(std::iter::once(user_id.to_string()))
                .collect::<Vec<String>>(),
            _ => disc
                .get_user_ids()
                .iter()
                .map(|id| id.id.to_raw())
                .collect::<Vec<String>>(),
        };

        let updated_discs_users = self
            .discussion_users
            .set_new_latest_post(
                &post.belongs_to.id.to_raw(),
                disc_all_users.iter().map(|id| id).collect::<Vec<&String>>(),
                &post.id.as_ref().unwrap().id.to_raw(),
                disc_all_users
                    .iter()
                    .filter(|id| id.as_str() != user_id)
                    .collect::<Vec<&String>>(),
            )
            .await?;

        if !post_data.tags.is_empty() {
            let _ = self
                .tags_repository
                .create_with_relate(post_data.tags, post.id.as_ref().unwrap().clone())
                .await?;
        }

        let post_view = PostView {
            id: post.id.as_ref().unwrap().clone(),
            created_by: UserView::from(user.clone()),
            belongs_to: post.belongs_to.clone(),
            title: post.title.clone(),
            content: post.content.clone(),
            media_links: post.media_links.clone(),
            created_at: post.created_at,
            updated_at: post.updated_at,
            replies_nr: post.replies_nr,
            likes_nr: post.likes_nr,
            liked_by: None,
            tasks_nr: 0,
            r#type: post.r#type.clone(),
            users: None,
        };

        let _ = self
            .notification_service
            .on_updated_users_discussions(&user.id.as_ref().unwrap(), &updated_discs_users)
            .await?;

        let _ = self
            .notification_service
            .on_discussion_post(&user_id, disc_all_users, &post_view)
            .await?;

        Ok(post_view)
    }

    pub async fn get(&self, user_id: &str, post_id: &str) -> AppResult<FullPostView> {
        let user = self.users_repository.get_by_id(&user_id).await?;
        let post = self
            .posts_repository
            .get_view_by_id::<PostAccessView>(post_id, None)
            .await?;

        if !PostAccess::new(&post).can_view(&user) {
            return Err(AppError::Forbidden.into());
        }

        self.posts_repository
            .get_view_by_id::<FullPostView>(post_id, Some(user_id))
            .await
            .map_err(|e| e.into())
    }

    pub async fn delete_post(&self, user_id: &str, post_id: &str) -> AppResult<()> {
        let user = self.users_repository.get_by_id(&user_id).await?;
        let post = self
            .posts_repository
            .get_view_by_id::<PostAccessView>(post_id, None)
            .await?;

        if !PostAccess::new(&post).can_delete(&user) || post.tasks_nr > 0 {
            return Err(AppError::Forbidden.into());
        }

        self.posts_repository.delete(&post.id.id.to_raw()).await?;

        if post.discussion.r#type == DiscussionType::Private {
            let disc_id = post.discussion.id.id.to_raw();
            let users = post
                .discussion
                .users
                .into_iter()
                .map(|u| u.user.id.to_raw())
                .collect::<Vec<String>>();
            self.discussion_users
                .update_latest_post(&disc_id, users)
                .await?;
        }

        Ok(())
    }

    async fn get_post_data_of_input(&self, data: PostInput) -> CtxResult<PostCreationData> {
        data.validate()?;
        if data.content.is_none() && data.file_1.is_none() {
            return Err(AppError::Generic {
                description: "Empty content and missing file".to_string(),
            }
            .into());
        }

        let (r#type, members) = if data.is_idea.unwrap_or_default() {
            (PostType::Idea, Vec::new())
        } else if !data.users.is_empty() {
            let members = self.get_users_by_ids(data.users).await?;
            (PostType::Private, members)
        } else {
            (PostType::Public, Vec::new())
        };

        Ok(PostCreationData {
            id: PostDbService::get_new_post_thing(),
            title: data.title,
            tags: data
                .tags
                .iter()
                .map(|t| t.to_lowercase())
                .collect::<Vec<String>>(),
            file: data
                .file_1
                .map(|v| convert_field_file_data(v))
                .transpose()?,
            members,
            r#type,
            content: data.content,
        })
    }

    async fn get_users_by_ids(&self, user_ids: Vec<String>) -> CtxResult<Vec<LocalUser>> {
        let user_things = user_ids
            .iter()
            .filter_map(|u| get_str_thing(&u).ok())
            .collect::<Vec<Thing>>();

        if user_things.is_empty() {
            return Err(AppError::Forbidden.into());
        }

        Ok(self.users_repository.get_by_ids(user_things).await?)
    }

    fn check_create_access(
        &self,
        disc: &DiscussionAccessView,
        data: &PostCreationData,
        user: &LocalUser,
    ) -> AppResult<()> {
        let disc_access = DiscussionAccess::new(disc);

        let members_access = data.members.iter().all(|u| disc_access.can_view(u));
        let owner_access = match &data.r#type {
            PostType::Public => disc_access.can_create_public_post(&user),
            PostType::Private => disc_access.can_create_private_post(&user),
            PostType::Idea => disc_access.can_idea_post(&user),
        };

        match (owner_access, members_access) {
            (true, true) => Ok(()),
            _ => Err(AppError::Forbidden.into()),
        }
    }
}
#[derive(Debug)]
struct PostCreationData {
    id: Thing,
    tags: Vec<String>,
    file: Option<FileUpload>,
    members: Vec<LocalUser>,
    r#type: PostType,
    content: Option<String>,
    title: String,
}
