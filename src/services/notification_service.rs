use serde_json::json;
use tokio::sync::broadcast::Sender;

use crate::database::client::Db;
use crate::entities::user_notification::UserNotificationEvent;
use crate::interfaces::repositories::user_notifications::UserNotificationsInterface;
use crate::middleware::mw_ctx::AppEventMetadata;
use crate::models::view::post::PostView;
use crate::models::view::reply::ReplyView;
use crate::{
    entities::user_auth::{follow_entity::FollowDbService, local_user_entity::LocalUser},
    middleware::{
        ctx::Ctx,
        error::{AppError, CtxResult},
        mw_ctx::{AppEvent, AppEventType},
    },
};

use surrealdb::sql::Thing;

pub struct NotificationService<'a, N>
where
    N: UserNotificationsInterface,
{
    follow_repository: FollowDbService<'a>,
    notification_repository: &'a N,
    event_sender: &'a Sender<AppEvent>,
    ctx: &'a Ctx,
}

impl<'a, N> NotificationService<'a, N>
where
    N: UserNotificationsInterface,
{
    pub fn new(
        db: &'a Db,
        ctx: &'a Ctx,
        event_sender: &'a Sender<AppEvent>,
        notification_repository: &'a N,
    ) -> NotificationService<'a, N> {
        NotificationService {
            follow_repository: FollowDbService { db, ctx },
            notification_repository,
            event_sender,
            ctx,
        }
    }

    pub async fn on_like(
        &self,
        user_id: &Thing,
        participators: Vec<Thing>,
        post_id: Thing,
    ) -> CtxResult<()> {
        let user_id_str = user_id.to_raw();
        let receivers = participators
            .iter()
            .map(|id| id.to_raw())
            .collect::<Vec<String>>();

        let event = self
            .notification_repository
            .create(
                &user_id_str,
                "like",
                &UserNotificationEvent::UserLikePost.as_str(),
                &receivers,
                Some(json!({
                    "user_id": user_id_str,
                    "post_id": post_id.to_raw()
                })),
            )
            .await?;

        let _ = self.event_sender.send(AppEvent {
            user_id: user_id_str,
            metadata: None,
            event: AppEventType::UserNotificationEvent(event),
            content: None,
            receivers,
        });

        Ok(())
    }

    pub async fn on_follow(
        &self,
        user: &LocalUser,
        follow_username: String,
        participators: Vec<Thing>,
    ) -> CtxResult<()> {
        let user_id_str = user.id.as_ref().unwrap().to_raw();
        let receivers = participators
            .iter()
            .map(|id| id.to_raw())
            .collect::<Vec<String>>();
        let event = self
            .notification_repository
            .create(
                &user_id_str,
                "follow",
                UserNotificationEvent::UserLikePost.as_str(),
                &receivers,
                Some(json!({
                    "username": user.username.clone(),
                    "follows_username": follow_username
                })),
            )
            .await?;
        let _ = self.event_sender.send(AppEvent {
            receivers,
            user_id: user_id_str,
            metadata: None,
            content: None,
            event: AppEventType::UserNotificationEvent(event),
        });

        Ok(())
    }

    pub async fn on_deliver_task(
        &self,
        user_id: &Thing,
        task_id: Thing,
        participators: &Vec<Thing>,
    ) -> CtxResult<()> {
        let user_id_str = user_id.to_raw();
        let receivers = participators
            .iter()
            .map(|id| id.to_raw())
            .collect::<Vec<String>>();
        let event = self
            .notification_repository
            .create(
                &user_id_str,
                "deliver",
                UserNotificationEvent::UserTaskRequestDelivered.as_str(),
                &receivers,
                Some({
                    json!({
                    "task_id": task_id.to_raw(),
                    "delivered_by": user_id.to_raw()})
                }),
            )
            .await?;

        let _ = self.event_sender.send(AppEvent {
            receivers,
            user_id: user_id_str,
            metadata: None,
            content: None,
            event: AppEventType::UserNotificationEvent(event),
        });

        Ok(())
    }

    pub async fn on_update_balance(&self, user_id: &Thing) -> CtxResult<()> {
        let user_id_str = user_id.to_raw();
        let receivers = vec![user_id.to_raw()];
        let event = self
            .notification_repository
            .create(
                &user_id_str,
                "update your balance",
                &UserNotificationEvent::UserBalanceUpdate.as_str(),
                &receivers,
                None,
            )
            .await?;

        let _ = self.event_sender.send(AppEvent {
            receivers,
            user_id: user_id_str,
            metadata: None,
            content: None,
            event: AppEventType::UserNotificationEvent(event),
        });

        Ok(())
    }

    pub async fn on_create_post(
        &self,
        user_id: &Thing,
        participators: &Vec<Thing>,
        post: &PostView,
    ) -> CtxResult<()> {
        let user_id_str = user_id.to_raw();
        let receivers = participators
            .iter()
            .map(|id| id.to_raw())
            .collect::<Vec<String>>();

        let event = self
            .notification_repository
            .create(
                &user_id_str,
                &format!("`{}` created the post", post.created_by.username),
                UserNotificationEvent::CreatedPost.as_str(),
                &receivers,
                Some({
                    json!({
                        "post_id": post.id,
                        "post_type": post.r#type
                    })
                }),
            )
            .await?;
        let _ = self.event_sender.send(AppEvent {
            receivers,
            user_id: user_id_str,
            metadata: None,
            content: None,
            event: AppEventType::UserNotificationEvent(event),
        });

        Ok(())
    }

    pub async fn on_created_task(
        &self,
        user_id: &Thing,
        task_id: &Thing,
        to_user: &Thing,
    ) -> CtxResult<()> {
        let user_id_str = user_id.to_raw();

        let follower_ids: Vec<Thing> = self
            .follow_repository
            .user_follower_ids(user_id.clone())
            .await?;

        let receivers = follower_ids
            .iter()
            .map(|id| id.to_raw())
            .collect::<Vec<String>>();

        let event = self
            .notification_repository
            .create(
                &user_id_str,
                "create a task",
                UserNotificationEvent::UserTaskRequestCreated.as_str(),
                &receivers,
                Some(json!({
                        "task_id": task_id.to_raw(),  
                        "from_user": user_id.to_raw(),
                        "to_user": to_user.to_raw()})),
            )
            .await?;

        let _ = self.event_sender.send(AppEvent {
            receivers,
            content: None,
            user_id: user_id_str,
            metadata: None,
            event: AppEventType::UserNotificationEvent(event),
        });

        Ok(())
    }

    pub async fn on_received_task(
        &self,
        user_id: &Thing,
        task_id: &Thing,
        to_user: &Thing,
    ) -> CtxResult<()> {
        let user_id_str = user_id.to_raw();

        let follower_ids: Vec<Thing> = self
            .follow_repository
            .user_follower_ids(to_user.clone())
            .await?;

        let receivers = follower_ids
            .iter()
            .map(|id| id.to_raw())
            .collect::<Vec<String>>();
        let event = self
            .notification_repository
            .create(
                &user_id_str,
                "create a task",
                UserNotificationEvent::UserTaskRequestReceived.as_str(),
                &receivers,
                Some(json!({
                        "task_id": task_id.to_raw(),  
                        "from_user": user_id.to_raw(),
                        "to_user": to_user.to_raw()})),
            )
            .await?;

        let _ = self.event_sender.send(AppEvent {
            receivers,
            content: None,
            user_id: user_id_str,
            metadata: None,
            event: AppEventType::UserNotificationEvent(event),
        });

        Ok(())
    }

    pub async fn on_discussion_post_reply(
        &self,
        user_id: &Thing,
        post_id: &Thing,
        discussion_id: &Thing,
        content: &ReplyView,
    ) -> CtxResult<()> {
        let user_id_str = user_id.to_raw();

        let follower_ids: Vec<Thing> = self
            .follow_repository
            .user_follower_ids(user_id.clone())
            .await?;

        let metadata = AppEventMetadata {
            discussion_id: Some(discussion_id.clone()),
            post_id: Some(post_id.clone()),
        };
        let _ = self.event_sender.send(AppEvent {
            receivers: follower_ids.iter().map(|id| id.to_raw()).collect(),
            user_id: user_id_str,
            content: Some(serde_json::to_string(&content).map_err(|_| {
                self.ctx.to_ctx_error(AppError::Generic {
                    description: "Reply to json error for notification event".to_string(),
                })
            })?),
            metadata: Some(metadata.clone()),
            event: AppEventType::DiscussionPostReplyAdded,
        });

        Ok(())
    }

    pub async fn on_discussion_post_reply_nr_increased(
        &self,
        user_id: &Thing,
        post_id: &Thing,
        discussion_id: &Thing,
        content: &String,
    ) -> CtxResult<()> {
        let user_id_str = user_id.to_raw();

        let follower_ids: Vec<Thing> = self
            .follow_repository
            .user_follower_ids(user_id.clone())
            .await?;

        let metadata = AppEventMetadata {
            discussion_id: Some(discussion_id.clone()),
            post_id: Some(post_id.clone()),
        };

        let receivers = follower_ids
            .iter()
            .map(|id| id.to_raw())
            .collect::<Vec<String>>();

        let _ = self.event_sender.send(AppEvent {
            receivers,
            user_id: user_id_str,
            metadata: Some(metadata),
            content: Some(content.clone()),
            event: AppEventType::DiscussionPostReplyNrIncreased,
        });

        Ok(())
    }

    pub async fn on_discussion_post(&self, user_id: &Thing, post: &PostView) -> CtxResult<()> {
        let receivers = vec![user_id.to_raw()];

        let post_json = serde_json::to_string(&post).map_err(|_| {
            self.ctx.to_ctx_error(AppError::Generic {
                description: "Post to json error for notification event".to_string(),
            })
        })?;

        let metadata = AppEventMetadata {
            discussion_id: Some(post.belongs_to.clone()),
            post_id: Some(post.id.clone()),
        };

        let _ = self.event_sender.send(AppEvent {
            user_id: user_id.to_raw(),
            event: AppEventType::DiscussionPostAdded,
            content: Some(post_json),
            receivers,
            metadata: Some(metadata),
        });

        Ok(())
    }
}
