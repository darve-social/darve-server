use crate::{
    database::client::Db,
    entities::{
        community::{
            discussion_entity::DiscussionDbService,
            post_entity::{Post, PostDbService},
            post_stream_entity::PostStreamDbService,
        },
        user_auth::{
            access_right_entity::AccessRightDbService,
            access_rule_entity::AccessRule,
            authorization_entity::{Authorization, AUTH_ACTIVITY_MEMBER},
            local_user_entity::LocalUserDbService,
        },
    },
    interfaces::{
        file_storage::FileStorageInterface,
        repositories::user_notifications::UserNotificationsInterface,
    },
    middleware::{
        ctx::Ctx,
        error::{AppError, CtxResult},
        mw_ctx::AppEvent,
        utils::{
            db_utils::{ViewFieldSelector, ViewRelateField},
            extractor_utils::DiscussionParams,
            string_utils::get_string_thing,
        },
    },
    services::notification_service::NotificationService,
    utils::file::convert::convert_field_file_data,
};

use axum_typed_multipart::{FieldData, TryFromMultipart};
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
    pub topic_id: Option<String>,
    #[validate(length(max = 5, message = "Max 5 tags"))]
    pub tags: Vec<String>,
    #[form_data(limit = "unlimited")]
    pub file_1: Option<FieldData<NamedTempFile>>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct PostView {
    pub id: Thing,
    pub created_by_name: String,
    pub belongs_to_uri: Option<String>,
    pub belongs_to_id: Thing,
    pub title: String,
    pub r_title_uri: Option<String>,
    pub content: String,
    pub media_links: Option<Vec<String>>,
    pub r_created: String,
    pub replies_nr: i64,
    pub access_rule: Option<AccessRule>,
    pub viewer_access_rights: Vec<Authorization>,
    pub has_view_access: bool,
}

impl ViewFieldSelector for PostView {
    // post fields selct qry for view
    fn get_select_query_fields() -> String {
        "id, created_by.username as created_by_name, title, r_title_uri, content, media_links, r_created, belongs_to.name_uri as belongs_to_uri, belongs_to.id as belongs_to_id, replies_nr, discussion_topic.{id, title} as topic, discussion_topic.access_rule.* as access_rule, [] as viewer_access_rights, false as has_view_access".to_string()
    }
}

impl ViewRelateField for PostView {
    fn get_fields() -> &'static str {
        "id,
        created_by_name: created_by.username, 
        title, 
        r_title_uri, 
        content,
        media_links, 
        r_created, 
        belongs_to_uri: belongs_to.name_uri, 
        belongs_to_id: belongs_to.id,
        replies_nr, 
        topic: discussion_topic.{id, title}, 
        access_rule: discussion_topic.access_rule.*, 
        viewer_access_rights: [], 
        has_view_access: false"
    }
}
pub struct PostService<'a, F, N>
where
    F: FileStorageInterface,
    N: UserNotificationsInterface,
{
    users_repository: LocalUserDbService<'a>,
    discussions_repository: DiscussionDbService<'a>,
    access_repository: AccessRightDbService<'a>,
    posts_repository: PostDbService<'a>,
    file_storage: &'a F,
    notification_service: NotificationService<'a, N>,
    streams_repository: PostStreamDbService<'a>,
}

impl<'a, F, N> PostService<'a, F, N>
where
    F: FileStorageInterface,
    N: UserNotificationsInterface,
{
    pub fn new(
        db: &'a Db,
        ctx: &'a Ctx,
        event_sender: &'a Sender<AppEvent>,
        notification_repository: &'a N,
        file_storage: &'a F,
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
            access_repository: AccessRightDbService { db: &db, ctx },
            streams_repository: PostStreamDbService { db: &db, ctx },
        }
    }

    pub async fn like(&self, post_id: &str, user_id: &str, data: PostLikeData) -> CtxResult<u32> {
        let user = self.users_repository.get_by_id(&user_id).await?;
        let post = self.posts_repository.get_by_id(post_id).await?;

        let count = data.count.unwrap_or(1);
        let likes_count = self
            .posts_repository
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
            .posts_repository
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
        let _ = self.users_repository.get_by_id(&user_id).await?;
        let disc = self.discussions_repository.get_by_id(disc_id).await?;

        let items = self
            .posts_repository
            .get_by_discussion_desc_view::<PostView>(disc.id.as_ref().unwrap().clone(), query)
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

            vec![result]
        } else {
            vec![]
        };

        let topic_val: Option<Thing> = match data.topic_id {
            Some(v) => Some(get_string_thing(v).map_err(|_| AppError::Generic {
                description: "Topic id is invalid".to_string(),
            })?),
            None => None,
        };

        let post_res = self
            .posts_repository
            .create_update(Post {
                id: Some(new_post_id),
                belongs_to: disc.id.clone().unwrap(),
                discussion_topic: topic_val.clone(),
                title: data.title,
                r_title_uri: None,
                content: data.content,
                media_links: if media_links.is_empty() {
                    None
                } else {
                    Some(media_links.clone())
                },
                metadata: None,
                r_created: None,
                created_by: user.id.clone().unwrap(),
                r_updated: None,
                r_replies: None,
                likes_nr: 0,
                replies_nr: 0,
                tags: if data.tags.is_empty() {
                    None
                } else {
                    Some(data.tags)
                },
                deny_rules: None,
            })
            .await;

        let post = match post_res {
            Ok(value) => value,
            Err(err) => {
                let futures = media_links.into_iter().map(|link| {
                    let file_storage = self.file_storage;
                    async move {
                        if let Some(filename) = link.split('/').last() {
                            let _ = file_storage.delete(Some("posts"), filename).await;
                        }
                    }
                });

                join_all(futures).await;
                return Err(err);
            }
        };
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
                    id: post.id.clone().unwrap(),
                    created_by_name: user.username.clone(),
                    belongs_to_uri: None,
                    belongs_to_id: post.belongs_to.clone(),
                    title: post.title.clone(),
                    r_title_uri: post.r_title_uri.clone(),
                    content: post.content.clone().unwrap_or_default(),
                    media_links: post.media_links.clone(),
                    r_created: post.r_created.clone().unwrap_or_default(),
                    replies_nr: post.replies_nr,
                    access_rule: None,
                    viewer_access_rights: vec![],
                    has_view_access: true,
                },
            )
            .await?;

        Ok(post)
    }
}
