use crate::{
    access::{base::role::Role, discussion::DiscussionAccess, post::PostAccess},
    database::client::Db,
    entities::{
        community::{
            discussion_entity::{DiscussionDbService, DiscussionType},
            post_entity::{CreatePost, Post, PostDbService, PostType},
            post_stream_entity::PostStreamDbService,
        },
        user_auth::local_user_entity::LocalUserDbService,
    },
    interfaces::{
        file_storage::FileStorageInterface,
        repositories::{
            access::AccessRepositoryInterface, like::LikesRepositoryInterface,
            tags::TagsRepositoryInterface, user_notifications::UserNotificationsInterface,
        },
    },
    middleware::{
        ctx::Ctx,
        error::{AppError, CtxResult},
        mw_ctx::AppEvent,
        utils::db_utils::{Pagination, QryOrder, ViewFieldSelector, ViewRelateField},
    },
    models::view::{
        access::{DiscussionAccessView, PostAccessView},
        user::UserView,
    },
    services::notification_service::NotificationService,
    utils::file::convert::convert_field_file_data,
};

use axum_typed_multipart::{FieldData, TryFromMultipart};
use chrono::{DateTime, Utc};
use futures::future::join_all;
use serde::{Deserialize, Serialize};
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

#[derive(Debug, Deserialize)]
pub struct PostLikeData {
    pub count: Option<u16>,
}

#[derive(Validate, TryFromMultipart)]
pub struct PostInput {
    #[validate(length(min = 5, message = "Min 5 characters"))]
    pub title: String,
    #[validate(length(min = 1, message = "Content cannot be empty"))]
    pub content: Option<String>,
    #[validate(length(max = 5, message = "Max 5 tags"))]
    pub tags: Vec<String>,
    #[form_data(limit = "unlimited")]
    pub file_1: Option<FieldData<NamedTempFile>>,
    pub is_idea: Option<bool>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct PostView {
    pub id: Thing,
    pub created_by: UserView,
    pub belongs_to: Thing,
    pub title: String,
    pub content: Option<String>,
    pub media_links: Option<Vec<String>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub replies_nr: i64,
    pub likes_nr: i64,
    pub liked_by: Option<Vec<Thing>>,
}

impl ViewFieldSelector for PostView {
    fn get_select_query_fields() -> String {
        "id,
        created_by.* as created_by, 
        title, 
        content,
        media_links, 
        created_at,
        updated_at,
        belongs_to,
        replies_nr,
        likes_nr,
        <-like[WHERE in=$user].in as liked_by"
            .to_string()
    }
}

impl ViewRelateField for PostView {
    fn get_fields() -> &'static str {
        "id,
        created_by: created_by.*, 
        title, 
        content,
        media_links, 
        created_at,
        updated_at,
        belongs_to,
        replies_nr,
        likes_nr,
        liked_by: <-like[WHERE in=$user].in"
    }
}
pub struct PostService<'a, F, N, T, L, A>
where
    F: FileStorageInterface,
    N: UserNotificationsInterface,
    T: TagsRepositoryInterface,
    L: LikesRepositoryInterface,
    A: AccessRepositoryInterface,
{
    users_repository: LocalUserDbService<'a>,
    discussions_repository: DiscussionDbService<'a>,
    streams_repository: PostStreamDbService<'a>,
    posts_repository: PostDbService<'a>,
    file_storage: &'a F,
    likes_repository: &'a L,
    notification_service: NotificationService<'a, N>,
    tags_repository: &'a T,
    access_repository: &'a A,
}

impl<'a, F, N, T, L, A> PostService<'a, F, N, T, L, A>
where
    F: FileStorageInterface,
    N: UserNotificationsInterface,
    T: TagsRepositoryInterface,
    L: LikesRepositoryInterface,
    A: AccessRepositoryInterface,
{
    pub fn new(
        db: &'a Db,
        ctx: &'a Ctx,
        event_sender: &'a Sender<AppEvent>,
        notification_repository: &'a N,
        file_storage: &'a F,
        tags_repository: &'a T,
        likes_repository: &'a L,
        access_repository: &'a A,
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
            streams_repository: PostStreamDbService { db: &db, ctx },
        }
    }

    pub async fn like(&self, post_id: &str, user_id: &str, data: PostLikeData) -> CtxResult<u32> {
        let user = self.users_repository.get_by_id(&user_id).await?;
        let post = self
            .posts_repository
            .get_view_by_id::<PostAccessView>(post_id)
            .await?;

        if !PostAccess::new(&post).can_like(&user) {
            return Err(AppError::Forbidden.into());
        }

        let count = data.count.unwrap_or(1);
        let likes_count = self
            .likes_repository
            .like(user.id.as_ref().unwrap().clone(), post.id.clone(), count)
            .await?;

        self.notification_service
            .on_like(
                &user.id.as_ref().unwrap(),
                vec![user.id.as_ref().unwrap().clone()],
                post.id.clone(),
            )
            .await?;

        Ok(likes_count)
    }

    pub async fn unlike(&self, post_id: &str, user_id: &str) -> CtxResult<u32> {
        let user = self.users_repository.get_by_id(&user_id).await?;
        let post = self
            .posts_repository
            .get_view_by_id::<PostAccessView>(post_id)
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

    pub async fn get_by_query(
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
            .get_by_query(
                &user.id.as_ref().unwrap().id.to_raw(),
                &disc.id.id.to_raw(),
                query.filter_by_type,
                pagination,
            )
            .await?;

        Ok(items)
    }

    pub async fn create(&self, user_id: &str, disc_id: &str, data: PostInput) -> CtxResult<Post> {
        data.validate()?;

        if data.content.is_none() && data.file_1.is_none() {
            return Err(AppError::Generic {
                description: "Empty content and missing file".to_string(),
            }
            .into());
        }

        let user = self.users_repository.get_by_id(user_id).await?;
        let disc = self
            .discussions_repository
            .get_view_by_id::<DiscussionAccessView>(disc_id)
            .await?;

        let post_type = if data.is_idea.unwrap_or_default() {
            PostType::Idea
        } else {
            PostType::Public
        };

        let has_access = match post_type {
            PostType::Public => DiscussionAccess::new(&disc).can_create_public_post(&user),
            PostType::Private => DiscussionAccess::new(&disc).can_create_private_post(&user),
            PostType::Idea => DiscussionAccess::new(&disc).can_idea_post(&user),
        };

        if !has_access {
            return Err(AppError::Forbidden.into());
        }

        let new_post_id = PostDbService::get_new_post_thing();
        let media_links = if let Some(uploaded_file) = data.file_1 {
            let file = convert_field_file_data(uploaded_file)?;

            let file_name = format!(
                "{}_{}",
                new_post_id.clone().to_raw().replace(":", "_"),
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
                belongs_to: disc.id,
                title: data.title,
                content: data.content,
                media_links: media_links.clone(),
                created_by: user.id.as_ref().unwrap().clone(),
                id: new_post_id,
                r#type: post_type,
            })
            .await;

        let post = match post_res {
            Ok(value) => value,
            Err(err) => {
                if let Some(links) = &media_links {
                    let futures = links.into_iter().map(|link| {
                        let file_storage = self.file_storage;
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

        if !data.tags.is_empty() {
            let _ = self
                .tags_repository
                .create_with_relate(data.tags, post.id.as_ref().unwrap().clone())
                .await?;
        }

        if disc.r#type == DiscussionType::Private {
            self.notification_service
                .on_chat_message(
                    &user.id.as_ref().unwrap(),
                    &disc
                        .users
                        .into_iter()
                        .map(|u| u.user)
                        .collect::<Vec<Thing>>(),
                    &post,
                )
                .await?;
        } else {
            self.notification_service
                .on_community_post(&user.id.as_ref().unwrap(), &post)
                .await?;

            self.streams_repository
                .to_user_follower_streams(
                    post.created_by.clone(),
                    &post.id.clone().expect("has id"),
                )
                .await?;
        }

        let _ = self
            .notification_service
            .on_discussion_post(
                &user.id.as_ref().unwrap(),
                &PostView {
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
                },
            )
            .await?;

        Ok(post)
    }
}
