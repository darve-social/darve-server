use serde_json::json;
use tokio::sync::broadcast::Sender;

use crate::database::client::Db;
use crate::entities::user_notification::{ UserNotificationEvent};
use crate::interfaces::repositories::user_notifications::UserNotificationsInterface;
use crate::middleware::mw_ctx::AppEventMetadata;
use crate::{
    entities::{
        user_auth::{follow_entity::FollowDbService, local_user_entity::LocalUser},
    },
    middleware::{
        ctx::Ctx,
        error::{AppError, CtxResult},
        mw_ctx::{AppEvent, AppEventType},
    },
    routes::community::{
      discussion_routes::DiscussionPostView,
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
        let receivers = participators.iter().map(|id| id.to_raw()).collect::<Vec<String>>();

        let event = self
            .notification_repository
            .create(
                &user_id_str,
                "like",
                &UserNotificationEvent::UserLikePost.as_str(),
                &receivers,
                None,
                Some(json!({ "user_id": user_id_str, "post_id": post_id })),
            )
            .await?;

        let _ = self.event_sender.send(AppEvent {
            user_id: user_id_str,
            metadata: None,
            event: AppEventType::UserNotificationEvent(event),
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
        let receivers = participators.iter().map(|id| id.to_raw()).collect::<Vec<String>>();
        let metadata = json!({ "username": user.username.clone(), "follows_username": follow_username });
        let event = self
            .notification_repository
            .create(
                &user_id_str,
                "follow",
                UserNotificationEvent::UserLikePost.as_str(),
                &receivers,
                None,
                Some(metadata.clone()),
            )
            .await?;
        let _ = self.event_sender.send(AppEvent {
            receivers,
            user_id: user_id_str,
            metadata: None,
            event: AppEventType::UserNotificationEvent(event),
        });

        Ok(())
    }

    pub async fn on_deliver_task(
        &self,
        user_id: &Thing,
        task_id: Thing,
        deliverable: Thing,
        participators: &Vec<Thing>,
    ) -> CtxResult<()> {
        let user_id_str = user_id.to_raw();
        let receivers = participators.iter().map(|id| id.to_raw()).collect::<Vec<String>>();
        let event = self
            .notification_repository
            .create(
                &user_id_str,
                "deliver",
                UserNotificationEvent::UserTaskRequestDelivered.as_str(),
                &receivers,
                None,
                Some({
                    json!({ "task_id": task_id.to_raw(), "deliverable": deliverable.to_raw(), "delivered_by": user_id.clone().to_raw()})
                }),
            )
            .await?;

        let _ = self.event_sender.send(AppEvent {
            receivers,
            user_id: user_id_str,
             metadata: None,
            event: AppEventType::UserNotificationEvent(event),
        });

        Ok(())
    }

    pub async fn on_update_balance(
        &self,
        user_id: &Thing,
        participators: &Vec<Thing>,
    ) -> CtxResult<()> {
        let user_id_str = user_id.to_raw();
        let receivers = participators.iter().map(|id| id.to_raw()).collect::<Vec<String>>();

        let event = self
            .notification_repository
            .create(
                &user_id_str,
                "deliver",
                &UserNotificationEvent::UserBalanceUpdate.as_str(),
                &receivers,
                None,
                None,
            )
            .await?;


        let _ = self.event_sender.send(AppEvent {
            receivers,
            user_id: user_id_str,
              metadata: None,
            event: AppEventType::UserNotificationEvent(event),
        });

        Ok(())
    }

    pub async fn on_chat_message(
        &self,
        user_id: &Thing,
        participators: &Vec<Thing>,
        content: &String,
    ) -> CtxResult<()> {
        let user_id_str = user_id.to_raw();
        let receivers = participators.iter().map(|id| id.to_raw()).collect::<Vec<String>>();

        let event = self
            .notification_repository
            .create(
                &user_id_str,
                "chat",
                UserNotificationEvent::UserChatMessage.as_str(),
                &receivers,
                Some(content.to_owned()),
                None,
            )
            .await?;
        let _ = self.event_sender.send(AppEvent {
            receivers,
            user_id: user_id_str,
           metadata: None,
            event: AppEventType::UserNotificationEvent(event),
        });

        Ok(())
    }

    pub async fn on_community_post(&self, user_id: &Thing, content: &String) -> CtxResult<()> {
        let user_id_str = user_id.to_raw();

        let follower_ids: Vec<Thing> = self
            .follow_repository
            .user_follower_ids(user_id.clone())
            .await?;

        let receivers = follower_ids.iter().map(|id| id.to_raw()).collect::<Vec<String>>();

        let event = self
            .notification_repository
            .create(
                &user_id_str,
                "chat",
                UserNotificationEvent::UserCommunityPost.as_str(),
                &receivers,
                Some(content.clone()),
                None,
            )
            .await?;

        let _ = self.event_sender.send(AppEvent {
            receivers,
            user_id: user_id_str,
               metadata: None,
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

        let receivers = follower_ids.iter().map(|id| id.to_raw()).collect::<Vec<String>>();

        let event = self
            .notification_repository
            .create(
                &user_id_str,
                "create a task",
                UserNotificationEvent::UserTaskRequestCreated.as_str(),
                &receivers,
                None,
                Some(
                    json!({ 
                        "task_id": task_id.clone(),  
                        "from_user": user_id.clone(),
                        "to_user": to_user.clone()}),
                ),
            )
            .await?;

        let _ = self.event_sender.send(AppEvent {
            receivers: receivers,
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

        let receivers = follower_ids.iter().map(|id| id.to_raw()).collect::<Vec<String>>();
        let event = self
            .notification_repository
            .create(
                &user_id_str,
                "create a task",
                UserNotificationEvent::UserTaskRequestReceived.as_str(),
                &receivers,
                None,
                Some(
                    json!({ 
                        "task_id": task_id.clone(),  
                        "from_user": user_id.clone(),
                        "to_user": to_user.clone()}),
                ),
            )
            .await?;

        let _ = self.event_sender.send(AppEvent {
            receivers: receivers,
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
        content: &String,
        topic_id: &Option<Thing>,
    ) -> CtxResult<()> {
        let user_id_str = user_id.to_raw();

        let follower_ids: Vec<Thing> = self
            .follow_repository
            .user_follower_ids(user_id.clone())
            .await?;

        let metadata =  AppEventMetadata {
            discussion_id: Some(discussion_id.clone()),
            topic_id: topic_id.clone(),
            post_id: Some(post_id.clone()),
        };
        let receivers = follower_ids.iter().map(|id| id.to_raw()).collect::<Vec<String>>();
        let event = self
            .notification_repository
            .create(
                &user_id_str,
                "create a task",
                UserNotificationEvent::DiscussionPostReplyAdded.as_str(),
                &receivers,
                Some(content.clone()),
                Some(json!(metadata)),
            )
            .await?;

        let _ = self.event_sender.send(AppEvent {
            receivers: follower_ids.iter().map(|id| id.to_raw()).collect(),
            user_id: user_id_str,
            metadata: Some(metadata.clone()),
            event: AppEventType::DiscussionNotificationEvent(event),
        });

        Ok(())
    }

    pub async fn on_discussion_post_reply_nr_increased(
        &self,
        user_id: &Thing,
        post_id: &Thing,
        discussion_id: &Thing,
        content: &String,
        topic_id: &Option<Thing>,
    ) -> CtxResult<()> {
        let user_id_str = user_id.to_raw();

        let follower_ids: Vec<Thing> = self
            .follow_repository
            .user_follower_ids(user_id.clone())
            .await?;

        let metadata =  AppEventMetadata {
            discussion_id: Some(discussion_id.clone()),
            topic_id: topic_id.clone(),
            post_id: Some(post_id.clone()),
        };

        let receivers = follower_ids.iter().map(|id| id.to_raw()).collect::<Vec<String>>();
        let event = self
            .notification_repository
            .create(
                &user_id_str,
                "post ",
                UserNotificationEvent::DiscussionPostReplyNrIncreased.as_str(),
                &receivers,
                 Some(content.clone()),
                Some(json!(metadata)),
            )
            .await?;


        let _ = self.event_sender.send(AppEvent {
            receivers,
            user_id: user_id_str,
            metadata: Some(metadata),
            event: AppEventType::DiscussionNotificationEvent(event),
        });

        Ok(())
    }

    pub async fn on_discussion_post(
        &self,
        user_id: &Thing,
        post_comm_view: &DiscussionPostView,
    ) -> CtxResult<()> {
        let user_id_str = user_id.to_raw();
        let receivers = vec![user_id.clone().to_raw()];

        let post_json = serde_json::to_string(&post_comm_view).map_err(|_| {
            self.ctx.to_ctx_error(AppError::Generic {
                description: "Post to json error for notification event".to_string(),
            })
        })?;
          let metadata =  AppEventMetadata {
            discussion_id: Some(post_comm_view.belongs_to_id.clone(),),
            topic_id: post_comm_view.topic.clone().map(|t| t.id),
            post_id:  Some(post_comm_view.id.clone()),
        };

        let event = self
            .notification_repository
            .create(
                &user_id_str,
                "post ",
                UserNotificationEvent::DiscussionPostAdded.as_str(),
                &receivers,
                Some(post_json.clone()),
                Some(json!(metadata)),
            )
            .await?;
        let _ = self.event_sender.send(AppEvent {
            user_id: user_id.clone().to_raw(),
            event: AppEventType::DiscussionNotificationEvent(event),
            receivers: receivers,
            metadata: Some(metadata),
        });

        Ok(())
    }
}
