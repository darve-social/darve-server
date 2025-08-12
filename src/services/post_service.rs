use crate::{
    database::client::Db,
    entities::{
        community::{
            discussion_entity::DiscussionDbService,
            post_entity::{CreatePost, Post, PostDbService},
            post_stream_entity::PostStreamDbService,
        },
        user_auth::{
            access_right_entity::AccessRightDbService,
            authorization_entity::{Authorization, AUTH_ACTIVITY_MEMBER},
            local_user_entity::LocalUserDbService,
        },
    },
    interfaces::{
        file_storage::FileStorageInterface,
        repositories::{
            like::LikesRepositoryInterface, tags::TagsRepositoryInterface,
            user_notifications::UserNotificationsInterface,
        },
    },
    middleware::{
        ctx::Ctx,
        error::{AppError, CtxResult},
        mw_ctx::AppEvent,
        utils::{
            db_utils::{Pagination, ViewFieldSelector, ViewRelateField},
            extractor_utils::DiscussionParams,
        },
    },
    models::view::user::UserView,
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
}

impl ViewFieldSelector for PostView {
    // post fields selct qry for view
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
        likes_nr"
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
        likes_nr"
    }
}
pub struct PostService<'a, F, N, T, L>
where
    F: FileStorageInterface,
    N: UserNotificationsInterface,
    T: TagsRepositoryInterface,
    L: LikesRepositoryInterface,
{
    users_repository: LocalUserDbService<'a>,
    discussions_repository: DiscussionDbService<'a>,
    access_repository: AccessRightDbService<'a>,
    posts_repository: PostDbService<'a>,
    file_storage: &'a F,
    likes_repository: &'a L,
    notification_service: NotificationService<'a, N>,
    streams_repository: PostStreamDbService<'a>,
    tags_repository: &'a T,
}

impl<'a, F, N, T, L> PostService<'a, F, N, T, L>
where
    F: FileStorageInterface,
    N: UserNotificationsInterface,
    T: TagsRepositoryInterface,
    L: LikesRepositoryInterface,
{
    pub fn new(
        db: &'a Db,
        ctx: &'a Ctx,
        event_sender: &'a Sender<AppEvent>,
        notification_repository: &'a N,
        file_storage: &'a F,
        tags_repository: &'a T,
        likes_repository: &'a L,
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
            access_repository: AccessRightDbService { db: &db, ctx },
            streams_repository: PostStreamDbService { db: &db, ctx },
        }
    }

    pub async fn like(&self, post_id: &str, user_id: &str, data: PostLikeData) -> CtxResult<u32> {
        let user = self.users_repository.get_by_id(&user_id).await?;
        let post = self.posts_repository.get_by_id(post_id).await?;

        let count = data.count.unwrap_or(1);
        let likes_count = self
            .likes_repository
            .like(
                user.id.as_ref().unwrap().clone(),
                post.id.as_ref().unwrap().clone(),
                count,
            )
            .await?;

        self.notification_service
            .on_like(
                &user.id.as_ref().unwrap(),
                vec![user.id.as_ref().unwrap().clone()],
                post.id.as_ref().unwrap().clone(),
            )
            .await?;

        Ok(likes_count)
    }

    pub async fn unlike(&self, post_id: &str, user_id: &str) -> CtxResult<u32> {
        let user = self.users_repository.get_by_id(&user_id).await?;
        let post = self.posts_repository.get_by_id(post_id).await?;

        let likes_count = self
            .likes_repository
            .unlike(
                user.id.as_ref().unwrap().clone(),
                post.id.as_ref().unwrap().clone(),
            )
            .await?;

        Ok(likes_count)
    }

    pub async fn get_by_query(
        &self,
        disc_id: &str,
        user_id: &str,
        query: DiscussionParams,
    ) -> CtxResult<Vec<PostView>> {
        let user = self.users_repository.get_by_id(&user_id).await?;
        let disc = self.discussions_repository.get_by_id(disc_id).await?;

        if !disc.is_profile() && !disc.is_member(&user.id.as_ref().unwrap()) {
            return Err(AppError::Forbidden.into());
        }

        let items = self
            .posts_repository
            .get_by_query(
                &user.id.as_ref().unwrap().id.to_raw(),
                &disc.id.as_ref().unwrap().id.to_raw(),
                Pagination {
                    order_by: None,
                    order_dir: None,
                    count: query.count.unwrap_or(20),
                    start: query.start.unwrap_or(0),
                },
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
        let disc = self.discussions_repository.get_by_id(disc_id).await?;

        let is_user_chat = match disc.private_discussion_user_ids {
            Some(ref ids) => ids.contains(&user.id.as_ref().unwrap()),
            None => false,
        };

        if !is_user_chat {
            let min_authorization = Authorization {
                authorize_record_id: disc.id.clone().unwrap().clone(),
                authorize_activity: AUTH_ACTIVITY_MEMBER.to_string(),
                authorize_height: 0,
            };
            self.access_repository
                .is_authorized(&user.id.as_ref().unwrap(), &min_authorization)
                .await?;
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
                belongs_to: disc.id.clone().unwrap(),
                title: data.title,
                content: data.content,
                media_links: media_links.clone(),
                created_by: user.id.as_ref().unwrap().clone(),
                id: new_post_id,
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

        if !data.tags.is_empty() {
            let _ = self
                .tags_repository
                .create_with_relate(data.tags, post.id.as_ref().unwrap().clone())
                .await?;
        }
        // set latest post
        self.discussions_repository
            .set_latest_post_id(disc.id.clone().unwrap(), post.id.clone().unwrap())
            .await?;

        if is_user_chat {
            self.notification_service
                .on_chat_message(
                    &user.id.as_ref().unwrap(),
                    &disc.private_discussion_user_ids.clone().unwrap(),
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
                },
            )
            .await?;

        Ok(post)
    }
}
